use git2::{BranchType, DiffOptions, Repository, Signature, StashFlags, StatusOptions};
use std::path::Path;
use std::process::Command;

use crate::models::{
    BranchInfo, CommitInfo, ConflictInfo, DiffInfo, FileStatus, RepositoryInfo, StageResult,
    StashInfo,
};

pub fn open_repository(path: &str) -> Result<Repository, String> {
    Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))
}

/// Executes a git command safely.
/// Prevents shell injection by using Command::args directly.
/// Sanitizes critical inputs like URLs and branch names in caller functions.
fn run_git_command(
    args: Vec<&str>,
    cwd: Option<&str>,
    envs: Vec<(&str, String)>,
) -> Result<String, String> {
    let mut command = Command::new("git");
    
    // Explicitly set NO_PAGER to avoid interactive sessions
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GIT_PAGER", "cat");
    
    command.args(&args);
    
    if let Some(path) = cwd {
        command.current_dir(path);
    }
    
    for (key, val) in envs {
        command.env(key, val);
    }

    let output = command
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Git command failed with status: {}", output.status)
        })
    }
}

fn is_safe_git_arg(arg: &str) -> bool {
    // Prevent common shell/command injection patterns and flag injection
    !arg.is_empty() && 
    !arg.starts_with('-') && 
    !arg.contains(' ') && 
    !arg.contains(';') && 
    !arg.contains('&') && 
    !arg.contains('|') && 
    !arg.contains('`') && 
    !arg.contains('$') &&
    !arg.contains('\\')
}

pub fn clone_repository(
    url: &str,
    path: &str,
    ssh_key_path: Option<&str>,
    _ssh_passphrase: Option<&str>,
) -> Result<Repository, String> {
    if url.contains(' ') || url.contains(';') || url.starts_with('-') {
        return Err("Invalid clone URL".to_string());
    }
    
    let mut envs = Vec::new();
    if let Some(key) = ssh_key_path {
        if !key.trim().is_empty() {
            let expanded_path = if key.starts_with("~/") {
                let home = std::env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
                key.replacen("~", &home, 1)
            } else {
                key.to_string()
            };
            // Escape double quotes in path to prevent injection in GIT_SSH_COMMAND
            let escaped_path = expanded_path.replace('"', "\\\"");
            envs.push((
                "GIT_SSH_COMMAND",
                format!("ssh -i \"{}\" -o IdentitiesOnly=yes", escaped_path),
            ));
        }
    }
    run_git_command(vec!["clone", "--", url, path], None, envs)?;
    open_repository(path)
}

pub fn get_repository_info(repo: &Repository) -> Result<RepositoryInfo, String> {
    let mut ahead = 0;
    let mut behind = 0;
    let mut current_branch = "unknown".to_string();

    match repo.head() {
        Ok(head) => {
            current_branch = if head.is_branch() {
                head.shorthand().unwrap_or("unknown").to_string()
            } else {
                "detached HEAD".to_string()
            };

            if head.is_branch() {
                if let (Some(local_name), Some(local_oid)) = (head.name(), head.target()) {
                    if let Ok(upstream) = repo.branch_upstream_name(local_name) {
                        if let Some(upstream_name) = upstream.as_str() {
                            if let Ok(upstream_ref) = repo.find_reference(upstream_name) {
                                if let Some(upstream_oid) = upstream_ref.target() {
                                    if let Ok((a, b)) = repo.graph_ahead_behind(local_oid, upstream_oid) {
                                        ahead = a;
                                        behind = b;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            // Probably an unborn branch or empty repo
            if let Ok(head_ref) = repo.find_reference("HEAD") {
                if let Some(name) = head_ref.symbolic_target() {
                    current_branch = name.strip_prefix("refs/heads/").unwrap_or(name).to_string();
                }
            }
        }
    }

    let statuses = repo
        .statuses(None)
        .map_err(|e| format!("Failed to get statuses: {}", e))?;

    let is_dirty = !statuses.is_empty();

    let mut path = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| repo.path().to_string_lossy().to_string());
    
    // 移除末尾斜線，確保路徑格式一致
    while path.ends_with('/') || path.ends_with('\\') {
        path.pop();
    }

    Ok(RepositoryInfo {
        path,
        current_branch,
        is_dirty,
        ahead,
        behind,
    })
}

pub fn get_status(repo: &Repository) -> Result<Vec<FileStatus>, String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get status: {}", e))?;

    let mut file_statuses = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry.path().unwrap_or("unknown").to_string();

        let status_str =
            if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
                if status.is_index_new() {
                    "added"
                } else if status.is_index_modified() {
                    "modified"
                } else {
                    "deleted"
                }
            } else if status.is_wt_new() {
                "untracked"
            } else if status.is_wt_modified() {
                "modified"
            } else if status.is_wt_deleted() {
                "deleted"
            } else {
                "unknown"
            };

        let staged =
            status.is_index_new() || status.is_index_modified() || status.is_index_deleted();

        file_statuses.push(FileStatus {
            path,
            status: status_str.to_string(),
            staged,
        });
    }

    Ok(file_statuses)
}

pub fn stage_files(repo: &Repository, paths: Vec<String>) -> Result<StageResult, String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    let workdir = repo.workdir().ok_or("No working directory found")?;
    let mut staged = Vec::new();
    let mut warnings = Vec::new();

    for path in paths {
        let full_path = workdir.join(&path);
        if full_path.exists() {
            match index.add_path(Path::new(&path)) {
                Ok(_) => staged.push(path),
                Err(e) => warnings.push(format!("Failed to stage '{}': {}", path, e)),
            }
        } else {
            // File was deleted externally — clean up index entry and record warning
            let _ = index.remove_path(Path::new(&path));
            warnings.push(format!("Skipped '{}': file not found (removed from index)", path));
        }
    }

    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(StageResult { staged, warnings })
}

pub fn unstage_files(repo: &Repository, paths: Vec<String>) -> Result<(), String> {
    let head = repo.head().ok();
    let commit = head.and_then(|h| h.peel_to_commit().ok());

    if let Some(c) = commit {
        // Try bulk reset first; if it fails, fall back to per-file reset
        if repo
            .reset_default(Some(c.as_object()), paths.iter().map(|s| s.as_str()))
            .is_err()
        {
            for path in &paths {
                let _ = repo.reset_default(
                    Some(c.as_object()),
                    std::iter::once(path.as_str()),
                );
            }
        }
    } else {
        // No commits yet, just remove from index
        let mut index = repo
            .index()
            .map_err(|e| format!("Failed to get index: {}", e))?;
        for path in paths {
            index.remove_path(Path::new(&path)).ok();
        }
        index
            .write()
            .map_err(|e| format!("Failed to write index: {}", e))?;
    }

    Ok(())
}

pub fn create_safety_ref(repo: &Repository, action_name: &str) -> Result<(), String> {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(()), // No HEAD yet, nothing to snapshot
    };
    let commit = head.peel_to_commit().map_err(|e| e.to_string())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Use a specific namespace for safety refs
    let ref_name = format!("refs/safety/{}/{}", action_name, timestamp);
    repo.reference(&ref_name, commit.id(), true, &format!("safety snapshot before {}", action_name))
        .map_err(|e| format!("Failed to create safety ref: {}", e))?;
    Ok(())
}

pub fn amend_last_commit(repo: &Repository, message: &str) -> Result<String, String> {
    create_safety_ref(repo, "amend")?;
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    let tree_id = index
        .write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;

    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("Failed to find tree: {}", e))?;

    let signature = repo
        .signature()
        .or_else(|_| Signature::now("User", "user@example.com"))
        .map_err(|e| format!("Failed to create signature: {}", e))?;

    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let last_commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to peel HEAD to commit: {}", e))?;

    let commit_id = last_commit
        .amend(
            Some("HEAD"),
            Some(&signature),
            Some(&signature),
            None,
            Some(message),
            Some(&tree),
        )
        .map_err(|e| format!("Failed to amend commit: {}", e))?;

    Ok(commit_id.to_string())
}

pub fn cherry_pick(repo: &Repository, sha: &str) -> Result<(), String> {
    create_safety_ref(repo, "cherry-pick")?;
    let commit = repo
        .find_commit(git2::Oid::from_str(sha).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Commit not found: {}", e))?;

    let mut opts = git2::CherrypickOptions::new();
    repo.cherrypick(&commit, Some(&mut opts))
        .map_err(|e| format!("Cherry-pick failed: {}", e))?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        return Err("Cherry-pick resulted in conflicts. Please resolve them.".to_string());
    }

    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let signature = repo.signature().map_err(|e| e.to_string())?;
    
    let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let parent = head.peel_to_commit().map_err(|e| format!("Failed to peel HEAD: {}", e))?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        commit.message().unwrap_or("Cherry-picked commit"),
        &tree,
        &[&parent],
    ).map_err(|e| e.to_string())?;

    repo.cleanup_state().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn revert_commit(repo: &Repository, sha: &str) -> Result<(), String> {
    create_safety_ref(repo, "revert")?;
    let commit = repo
        .find_commit(git2::Oid::from_str(sha).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Commit not found: {}", e))?;

    let mut opts = git2::RevertOptions::new();
    repo.revert(&commit, Some(&mut opts))
        .map_err(|e| format!("Revert failed: {}", e))?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        return Err("Revert resulted in conflicts. Please resolve them.".to_string());
    }

    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let signature = repo.signature().map_err(|e| e.to_string())?;
    
    let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let parent = head.peel_to_commit().map_err(|e| format!("Failed to peel HEAD: {}", e))?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &format!("Revert \"{}\"", commit.message().unwrap_or("")),
        &tree,
        &[&parent],
    ).map_err(|e| e.to_string())?;

    repo.cleanup_state().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn discard_changes(repo: &Repository, path: &str) -> Result<(), String> {
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force().path(path);

    // Attempt checkout from HEAD
    if repo.checkout_head(Some(&mut checkout_opts)).is_err() {
        // If checkout head fails (e.g. untracked file), try to remove it
        let full_path = repo.workdir().ok_or("No workdir")?.join(path);
        if full_path.exists() {
            if full_path.is_file() {
                std::fs::remove_file(full_path)
                    .map_err(|e| format!("Failed to delete file: {}", e))?;
            } else if full_path.is_dir() {
                std::fs::remove_dir_all(full_path)
                    .map_err(|e| format!("Failed to delete dir: {}", e))?;
            }
        }
    }

    Ok(())
}

pub fn discard_all_changes(repo: &Repository) -> Result<(), String> {
    let _ = create_safety_ref(repo, "discard-all");
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_head(Some(&mut checkout_opts))
        .map_err(|e| format!("Failed to discard all changes: {}", e))
}


pub fn create_branch(repo: &Repository, name: &str) -> Result<(), String> {
    if !is_safe_git_arg(name) {
        return Err("Invalid branch name".to_string());
    }
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to peel HEAD to commit: {}", e))?;

    repo.branch(name, &commit, false)
        .map_err(|e| format!("Failed to create branch: {}", e))?;

    checkout_branch(repo, name)
}

pub fn get_commit_diff(repo: &Repository, sha: &str) -> Result<Vec<DiffInfo>, String> {
    let commit = repo
        .find_commit(git2::Oid::from_str(sha).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Commit not found: {}", e))?;

    let tree = commit
        .tree()
        .map_err(|e| format!("Failed to get tree: {}", e))?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .map_err(|e| e.to_string())? 
                .tree()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let mut diff_opts = DiffOptions::new();
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
        .map_err(|e| format!("Failed to generate diff: {}", e))?;

    let mut diff_infos = Vec::new();
    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let path = delta
            .new_file()
            .path()
            .and_then(|p| p.to_str())
            .unwrap_or("unknown")
            .to_string();

        let line_content = String::from_utf8_lossy(line.content()).to_string();
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            _ => "",
        };

        if let Some(info) = diff_infos
            .iter_mut()
            .find(|i: &&mut DiffInfo| i.path == path)
        {
            info.diff_text
                .push_str(&format!("{}{}", prefix, line_content));
            match line.origin() {
                '+' => info.additions += 1,
                '-' => info.deletions += 1,
                _ => {} 
            }
        } else {
            diff_infos.push(DiffInfo {
                path,
                diff_text: format!("{}{}", prefix, line_content),
                additions: if line.origin() == '+' { 1 } else { 0 },
                deletions: if line.origin() == '-' { 1 } else { 0 },
            });
        }
        true
    })
    .map_err(|e| format!("Failed to parse diff: {}", e))?;

    Ok(diff_infos)
}

pub fn create_commit(repo: &Repository, message: &str) -> Result<String, String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    let tree_id = index
        .write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;

    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("Failed to find tree: {}", e))?;

    let signature = repo
        .signature()
        .or_else(|_| Signature::now("User", "user@example.com"))
        .map_err(|e| format!("Failed to create signature: {}", e))?;

    let head = repo.head().ok();
    let parent_commit = head.as_ref().and_then(|h| h.peel_to_commit().ok());

    let parents = if let Some(ref parent) = parent_commit {
        vec![parent]
    } else {
        vec![]
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().map(|c| *c).collect();
    let commit_id = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .map_err(|e| format!("Failed to create commit: {}", e))?;

    Ok(commit_id.to_string())
}

pub fn get_branches(repo: &Repository) -> Result<Vec<BranchInfo>, String> {
    let branches = repo
        .branches(Some(BranchType::Local))
        .map_err(|e| format!("Failed to get branches: {}", e))?;

    let head = repo.head().ok();
    let current_branch_name = head
        .as_ref()
        .and_then(|h| h.shorthand())
        .map(|s| s.to_string());

    let mut branch_list = Vec::new();

    for branch_result in branches {
        let (branch, _) = branch_result.map_err(|e| format!("Failed to read branch: {}", e))?;
        let name = branch
            .name()
            .map_err(|e| format!("Failed to get branch name: {}", e))?
            .unwrap_or("unknown")
            .to_string();

        let is_current = current_branch_name.as_ref() == Some(&name);

        branch_list.push(BranchInfo {
            name,
            is_current,
            is_remote: false,
        });
    }

    Ok(branch_list)
}

pub fn checkout_branch(repo: &Repository, name: &str) -> Result<(), String> {
    if !is_safe_git_arg(name) {
        return Err("Invalid branch name".to_string());
    }
    let obj = repo
        .revparse_single(&format!("refs/heads/{}", name))
        .map_err(|e| format!("Failed to find branch: {}", e))?;

    repo.checkout_tree(&obj, None)
        .map_err(|e| format!("Failed to checkout tree: {}", e))?;

    repo.set_head(&format!("refs/heads/{}", name))
        .map_err(|e| format!("Failed to set HEAD: {}", e))?;

    Ok(())
}

pub fn get_commit_history(repo: &Repository, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let head = repo.head().ok();
    
    // Get upstream OID to check for pushed status
    let upstream_oid = head.as_ref().and_then(|h| {
        if h.is_branch() {
            h.name().and_then(|name| {
                repo.branch_upstream_name(name).ok().and_then(|upstream| {
                    upstream.as_str().and_then(|u_name| repo.find_reference(u_name).ok()).and_then(|r| r.target())
                })
            })
        } else {
            None
        }
    });

    let mut revwalk = repo
        .revwalk()
        .map_err(|e| format!("Failed to create revwalk: {}", e))?;

    revwalk
        .push_head()
        .map_err(|e| format!("Failed to push HEAD: {}", e))?;

    let mut commits = Vec::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid.map_err(|e| format!("Failed to get OID: {}", e))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| format!("Failed to find commit: {}", e))?;

        // Logic: if upstream can reach this commit, it is pushed.
        let is_pushed = if let Some(u_oid) = upstream_oid {
            repo.graph_descendant_of(u_oid, oid).unwrap_or(false) || u_oid == oid
        } else {
            false
        };

        commits.push(CommitInfo {
            sha: commit.id().to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("Unknown").to_string(),
            email: commit.author().email().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            is_pushed,
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
        });
    }

    Ok(commits)
}

pub fn get_diff(repo: &Repository, path: Option<&str>) -> Result<Vec<DiffInfo>, String> {
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    if let Some(p) = path {
        opts.pathspec(p);
    }

    let diff = if let Some(tree) = head_tree {
        repo.diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .map_err(|e| format!("Failed to get diff (tree to workdir): {}", e))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))
            .map_err(|e| format!("Failed to get diff (index to workdir): {}", e))?
    };

    let mut diff_infos = Vec::new();

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let file_path = delta
            .new_file()
            .path()
            .and_then(|p| p.to_str())
            .unwrap_or("unknown")
            .to_string();

        let line_content = String::from_utf8_lossy(line.content()).to_string();
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            _ => "",
        };

        if let Some(info) = diff_infos.iter_mut().find(|i: &&mut DiffInfo| i.path == file_path) {
            info.diff_text.push_str(&format!("{}{}", prefix, line_content));
            match line.origin() {
                '+' => info.additions += 1,
                '-' => info.deletions += 1,
                _ => {}
            }
        } else {
            diff_infos.push(DiffInfo {
                path: file_path,
                diff_text: format!("{}{}", prefix, line_content),
                additions: if line.origin() == '+' { 1 } else { 0 },
                deletions: if line.origin() == '-' { 1 } else { 0 },
            });
        }
        true
    })
    .map_err(|e| format!("Failed to parse diff: {}", e))?;

    Ok(diff_infos)
}

pub fn push_changes(
    repo: &Repository,
    ssh_key_path: Option<&str>,
    _ssh_passphrase: Option<&str>,
) -> Result<(), String> {
    let path = repo
        .workdir()
        .ok_or("No working directory found")?
        .to_str()
        .ok_or("Invalid path")?;
    let mut envs = Vec::new();
    if let Some(key) = ssh_key_path {
        if !key.trim().is_empty() {
            let expanded_path = if key.starts_with("~/") {
                key.replacen("~", &std::env::var("HOME").unwrap_or_default(), 1)
            } else {
                key.to_string()
            };
            envs.push((
                "GIT_SSH_COMMAND",
                format!("ssh -i \"{}\" -o IdentitiesOnly=yes", expanded_path),
            ));
        }
    }

    run_git_command(vec!["push", "origin", "HEAD"], Some(path), envs)?;
    Ok(())
}

pub fn pull_changes(
    repo: &Repository,
    ssh_key_path: Option<&str>,
    _ssh_passphrase: Option<&str>,
) -> Result<(), String> {
    let path = repo
        .workdir()
        .ok_or("No working directory found")?
        .to_str()
        .ok_or("Invalid path")?;
    let mut envs = Vec::new();
    if let Some(key) = ssh_key_path {
        if !key.trim().is_empty() {
            let expanded_path = if key.starts_with("~/") {
                key.replacen("~", &std::env::var("HOME").unwrap_or_default(), 1)
            } else {
                key.to_string()
            };
            envs.push((
                "GIT_SSH_COMMAND",
                format!("ssh -i \"{}\" -o IdentitiesOnly=yes", expanded_path),
            ));
        }
    }

    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let branch_name = if head.is_branch() {
        head.shorthand().unwrap_or("HEAD")
    } else {
        "HEAD"
    };

    run_git_command(vec!["pull", "origin", branch_name], Some(path), envs)?;
    Ok(())
}

pub fn stash_save(repo: &mut Repository, message: Option<&str>) -> Result<(), String> {
    let signature = repo
        .signature()
        .or_else(|_| Signature::now("User", "user@example.com"))
        .map_err(|e| format!("Failed to create signature: {}", e))?;

    repo.stash_save(
        &signature,
        message.unwrap_or(""),
        Some(StashFlags::INCLUDE_UNTRACKED),
    )
    .map_err(|e| format!("Failed to stash: {}", e))?;

    Ok(())
}

pub fn stash_pop(repo: &mut Repository, index: usize) -> Result<(), String> {
    repo.stash_pop(index, None)
        .map_err(|e| format!("Failed to pop stash: {}", e))?;
    Ok(())
}

pub fn stash_list(repo: &mut Repository) -> Result<Vec<StashInfo>, String> {
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, message, id| {
        stashes.push(StashInfo {
            index,
            message: message.to_string(),
            sha: id.to_string(),
        });
        true
    })
    .map_err(|e| format!("Failed to list stashes: {}", e))?;

    Ok(stashes)
}

pub fn get_conflicts(repo: &Repository) -> Result<Vec<ConflictInfo>, String> {
    let index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    let mut conflicts = Vec::new();
    for conflict in index
        .conflicts()
        .map_err(|e| format!("Failed to get conflicts: {}", e))? {
        let conflict = conflict.map_err(|e| format!("Conflict error: {}", e))?;
        let path = conflict
            .ancestor
            .as_ref()
            .or(conflict.our.as_ref())
            .or(conflict.their.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .unwrap_or_default();

        conflicts.push(ConflictInfo {
            path,
            our_status: if conflict.our.is_some() {
                "modified"
            } else {
                "deleted"
            }
            .to_string(),
            their_status: if conflict.their.is_some() {
                "modified"
            } else {
                "deleted"
            }
            .to_string(),
        });
    }

    Ok(conflicts)
}

pub fn resolve_conflict(repo: &Repository, path: &str, _use_ours: bool) -> Result<(), String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    index
        .add_path(Path::new(path))
        .map_err(|e| format!("Failed to resolve: {}", e))?;
    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

#[allow(dead_code)]
pub fn create_remote_callbacks() -> () {
    // Deprecated
}
pub fn fetch_changes(
    repo: &Repository,
    ssh_key_path: Option<&str>,
    _ssh_passphrase: Option<&str>,
) -> Result<(), String> {
    let path = repo
        .workdir()
        .ok_or("No working directory found")?
        .to_str()
        .ok_or("Invalid path")?;
    let mut envs = Vec::new();
    if let Some(key) = ssh_key_path {
        if !key.trim().is_empty() {
            let expanded_path = if key.starts_with("~/") {
                key.replacen("~", &std::env::var("HOME").unwrap_or_default(), 1)
            } else {
                key.to_string()
            };
            envs.push((
                "GIT_SSH_COMMAND",
                format!("ssh -i \"{}\" -o IdentitiesOnly=yes", expanded_path),
            ));
        }
    }

    run_git_command(vec!["fetch", "origin"], Some(path), envs)?;
    Ok(())
}

pub fn get_remote_url(repo: &Repository, name: &str) -> Result<String, String> {
    let remote = repo
        .find_remote(name)
        .map_err(|e| format!("Failed to find remote: {}", e))?;
    Ok(remote.url().unwrap_or("").to_string())
}

pub fn set_remote_url(repo: &Repository, name: &str, url: &str) -> Result<(), String> {
    repo.remote_set_url(name, url)
        .map_err(|e| format!("Failed to set remote URL: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn get_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push("tauri_git_test");
        path.push(format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_pull_changes() {
        // Setup origin
        let root = get_temp_dir();
        let origin_path = root.join("origin");
        let local_path = root.join("local");

        fs::create_dir(&origin_path).unwrap();
        let _ = Repository::init(&origin_path).unwrap();
        
        // Initial commit in origin
        run_git_command(vec!["init"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["config", "user.name", "Test User"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["config", "user.email", "test@example.com"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["commit", "--allow-empty", "-m", "Initial commit"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();
        
        // Create a branch 'feature'
        run_git_command(vec!["checkout", "-b", "feature"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();

        // Clone to local
        run_git_command(vec!["clone", origin_path.to_str().unwrap(), local_path.to_str().unwrap()], None, vec![]).unwrap();
        let local = Repository::open(&local_path).unwrap();
        run_git_command(vec!["config", "user.name", "Test User"], Some(local_path.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["config", "user.email", "test@example.com"], Some(local_path.to_str().unwrap()), vec![]).unwrap();

        // Switch local to feature branch (needs fetch first usually but clone gets all)
        // Checkout feature branch tracking origin/feature
        // origin was on 'feature', so clone checked it out. We just ensure we are on it.
        let _ = run_git_command(vec!["checkout", "feature"], Some(local_path.to_str().unwrap()), vec![]);

        // Add commit to origin/feature
        let file_path = origin_path.join("new_file.txt");
        fs::write(&file_path, "content").unwrap();
        run_git_command(vec!["add", "new_file.txt"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["commit", "-m", "Feature commit"], Some(origin_path.to_str().unwrap()), vec![]).unwrap();

        // Run pull_changes
        let result = pull_changes(&local, None, None);
        assert!(result.is_ok(), "pull_changes failed: {:?}", result.err());

        // Verify local has the commit
        let head = local.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert_eq!(commit.message().unwrap().trim(), "Feature commit");

        // Cleanup
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_amend_commit() {
        let root = get_temp_dir();
        let _ = Repository::init(&root).unwrap();
        let repo = Repository::open(&root).unwrap();

        run_git_command(vec!["config", "user.name", "Test User"], Some(root.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["config", "user.email", "test@example.com"], Some(root.to_str().unwrap()), vec![]).unwrap();

        // Initial commit
        let file_path = root.join("file.txt");
        fs::write(&file_path, "v1").unwrap();
        run_git_command(vec!["add", "."], Some(root.to_str().unwrap()), vec![]).unwrap();
        create_commit(&repo, "Initial commit").unwrap();

        // Amend
        let result = amend_last_commit(&repo, "Amended message");
        assert!(result.is_ok());

        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert_eq!(commit.message().unwrap(), "Amended message");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_discard_all_changes() {
        let root = get_temp_dir();
        let _ = Repository::init(&root).unwrap();
        let repo = Repository::open(&root).unwrap();

        run_git_command(vec!["config", "user.name", "Test User"], Some(root.to_str().unwrap()), vec![]).unwrap();
        run_git_command(vec!["config", "user.email", "test@example.com"], Some(root.to_str().unwrap()), vec![]).unwrap();
        
        fs::write(root.join("file.txt"), "v1").unwrap();
        run_git_command(vec!["add", "."], Some(root.to_str().unwrap()), vec![]).unwrap();
        create_commit(&repo, "Init").unwrap();

        // Modify file
        fs::write(root.join("file.txt"), "v2").unwrap();
        
        // Discard
        discard_all_changes(&repo).unwrap();

        let content = fs::read_to_string(root.join("file.txt")).unwrap();
        assert_eq!(content, "v1");

        let _ = fs::remove_dir_all(root);
    }
}
