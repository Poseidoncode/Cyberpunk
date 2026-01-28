use git2::{
    BranchType, Cred, DiffOptions, RemoteCallbacks, Repository, Signature, StashFlags,
    StatusOptions,
};
use std::path::Path;

use crate::models::{
    BranchInfo, ConflictInfo, CommitInfo, DiffInfo, FileStatus, RepositoryInfo, StashInfo,
};

pub fn open_repository(path: &str) -> Result<Repository, String> {
    Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))
}

pub fn clone_repository(url: &str, path: &str, ssh_key_path: Option<&str>) -> Result<Repository, String> {
    let mut callbacks = RemoteCallbacks::new();
    if let Some(key_path) = ssh_key_path {
        let key_path_owned = key_path.to_string();
        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&key_path_owned),
                None,
            )
        });
    }

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_opts);

    builder.clone(url, Path::new(path)).map_err(|e| format!("Failed to clone repository: {}", e))
}

pub fn get_repository_info(repo: &Repository) -> Result<RepositoryInfo, String> {
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    
    let current_branch = if head.is_branch() {
        head.shorthand().unwrap_or("unknown").to_string()
    } else {
        "detached HEAD".to_string()
    };

    let statuses = repo
        .statuses(None)
        .map_err(|e| format!("Failed to get statuses: {}", e))?;
    
    let is_dirty = !statuses.is_empty();

    Ok(RepositoryInfo {
        path: repo.path().to_string_lossy().to_string(),
        current_branch,
        is_dirty,
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

        let status_str = if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
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

        let staged = status.is_index_new() || status.is_index_modified() || status.is_index_deleted();

        file_statuses.push(FileStatus {
            path,
            status: status_str.to_string(),
            staged,
        });
    }

    Ok(file_statuses)
}

pub fn stage_files(repo: &Repository, paths: Vec<String>) -> Result<(), String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    for path in paths {
        index
            .add_path(Path::new(&path))
            .map_err(|e| format!("Failed to stage {}: {}", path, e))?;
    }

    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

pub fn unstage_files(repo: &Repository, paths: Vec<String>) -> Result<(), String> {
    let head = repo.head().ok();
    let commit = head.and_then(|h| h.peel_to_commit().ok());
    
    if let Some(c) = commit {
        repo.reset_default(Some(c.as_object()), paths.iter().map(|s| s.as_str()))
            .map_err(|e| format!("Failed to unstage: {}", e))?;
    } else {
        // No commits yet, just remove from index
        let mut index = repo.index().map_err(|e| format!("Failed to get index: {}", e))?;
        for path in paths {
            index.remove_path(Path::new(&path)).ok();
        }
        index.write().map_err(|e| format!("Failed to write index: {}", e))?;
    }

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
                std::fs::remove_file(full_path).map_err(|e| format!("Failed to delete file: {}", e))?;
            } else if full_path.is_dir() {
                std::fs::remove_dir_all(full_path).map_err(|e| format!("Failed to delete dir: {}", e))?;
            }
        }
    }
    
    Ok(())
}

pub fn create_branch(repo: &Repository, name: &str) -> Result<(), String> {
    let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
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

    let tree = commit.tree().map_err(|e| format!("Failed to get tree: {}", e))?;
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

        if let Some(info) = diff_infos.iter_mut().find(|i: &&mut DiffInfo| i.path == path) {
            info.diff_text.push_str(&format!("{}{}", prefix, line_content));
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
    let parent_commit = head
        .as_ref()
        .and_then(|h| h.peel_to_commit().ok());

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

        commits.push(CommitInfo {
            sha: commit.id().to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("Unknown").to_string(),
            email: commit.author().email().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
        });
    }

    Ok(commits)
}

pub fn get_diff(repo: &Repository, path: Option<&str>) -> Result<Vec<DiffInfo>, String> {
    let head_tree = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    if let Some(p) = path {
        opts.pathspec(p);
    }

    let diff = if let Some(tree) = head_tree {
        repo.diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .map_err(|e| format!("Failed to get diff: {}", e))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))
            .map_err(|e| format!("Failed to get diff: {}", e))?
    };

    let mut diff_infos = Vec::new();

    diff.foreach(
        &mut |delta, _| {
            let file_path = delta.new_file().path().unwrap_or(Path::new("unknown"));
            diff_infos.push(DiffInfo {
                path: file_path.to_string_lossy().to_string(),
                additions: 0,
                deletions: 0,
                diff_text: String::new(),
            });
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| format!("Failed to iterate diff: {}", e))?;

    Ok(diff_infos)
}

pub fn push_changes(repo: &Repository, ssh_key_path: Option<&str>) -> Result<(), String> {
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;

    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    
    let branch_name = head.shorthand().unwrap_or("main");
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    let mut callbacks = RemoteCallbacks::new();
    if let Some(key_path) = ssh_key_path {
        let key_path_owned = key_path.to_string();
        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&key_path_owned),
                None,
            )
        });
    }

    let mut push_opts = git2::PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    remote
        .push(&[&refspec], Some(&mut push_opts))
        .map_err(|e| format!("Failed to push: {}", e))?;

    Ok(())
}

pub fn pull_changes(repo: &Repository, ssh_key_path: Option<&str>) -> Result<(), String> {
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;

    let mut callbacks = RemoteCallbacks::new();
    if let Some(key_path) = ssh_key_path {
        let key_path_owned = key_path.to_string();
        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&key_path_owned),
                None,
            )
        });
    }

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    remote
        .fetch(&["main"], Some(&mut fetch_opts), None)
        .map_err(|e| format!("Failed to fetch: {}", e))?;

    // Simple fast-forward merge
    let fetch_head = repo
        .find_reference("FETCH_HEAD")
        .map_err(|e| format!("Failed to find FETCH_HEAD: {}", e))?;
    
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| format!("Failed to get fetch commit: {}", e))?;

    let analysis = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Failed to analyze merge: {}", e))?;

    if analysis.0.is_up_to_date() {
        return Ok(());
    } else if analysis.0.is_fast_forward() {
        let refname = "refs/heads/main";
        let mut reference = repo
            .find_reference(refname)
            .map_err(|e| format!("Failed to find reference: {}", e))?;
        
        reference
            .set_target(fetch_commit.id(), "Fast-forward")
            .map_err(|e| format!("Failed to set target: {}", e))?;
        
        repo.set_head(refname)
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Failed to checkout: {}", e))?;
    }

    Ok(())
}

pub fn stash_save(repo: &mut Repository, message: Option<&str>) -> Result<(), String> {
    let signature = repo
        .signature()
        .or_else(|_| Signature::now("User", "user@example.com"))
        .map_err(|e| format!("Failed to create signature: {}", e))?;

    repo.stash_save(&signature, message.unwrap_or(""), Some(StashFlags::INCLUDE_UNTRACKED))
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
    for conflict in index.conflicts().map_err(|e| format!("Failed to get conflicts: {}", e))? {
        let conflict = conflict.map_err(|e| format!("Conflict error: {}", e))?;
        let path = conflict.ancestor.as_ref().or(conflict.our.as_ref()).or(conflict.their.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .unwrap_or_default();
        
        conflicts.push(ConflictInfo {
            path,
            our_status: if conflict.our.is_some() { "modified" } else { "deleted" }.to_string(),
            their_status: if conflict.their.is_some() { "modified" } else { "deleted" }.to_string(),
        });
    }

    Ok(conflicts)
}

pub fn resolve_conflict(repo: &Repository, path: &str, _use_ours: bool) -> Result<(), String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;
    
    index.add_path(Path::new(path)).map_err(|e| format!("Failed to resolve: {}", e))?;
    index.write().map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

#[allow(dead_code)]
pub fn create_remote_callbacks() -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap_or("git"),
            None,
            std::path::Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap())),
            None,
        )
    });
    callbacks
}
pub fn fetch_changes(repo: &Repository, ssh_key_path: Option<&str>) -> Result<(), String> {
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;

    let mut callbacks = RemoteCallbacks::new();
    if let Some(key_path) = ssh_key_path {
        let key_path_owned = key_path.to_string();
        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&key_path_owned),
                None,
            )
        });
    }

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    remote.fetch(&["main"], Some(&mut fetch_opts), None)
        .map_err(|e| format!("Failed to fetch: {}", e))?;
        
    Ok(())
}

pub fn set_remote_url(repo: &Repository, name: &str, url: &str) -> Result<(), String> {
    repo.remote_set_url(name, url)
        .map_err(|e| format!("Failed to set remote URL: {}", e))?;
    Ok(())
}
