mod git_operations;
mod models;

use models::{
    BranchInfo, BranchOptions, CloneOptions, CommitInfo, CommitOptions, ConflictInfo, DiffInfo,
    FileStatus, RepositoryInfo, Settings, StageResult, StashInfo, StashOptions,
};
use notify::{Config, RecursiveMode, Watcher};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

struct AppState {
    repo: Option<git2::Repository>,
    settings: Settings,
    watcher: Option<notify::RecommendedWatcher>,
}

struct App(Mutex<AppState>);

fn start_watcher(app_handle: tauri::AppHandle, repo_path: &str) -> Option<notify::RecommendedWatcher> {
    let path = std::path::Path::new(repo_path);
    let git_path = path.join(".git");

    if !git_path.exists() {
        return None;
    }

    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = notify::RecommendedWatcher::new(tx, Config::default()).ok()?;

    // Watch key git files for state changes
    let _ = watcher.watch(&git_path.join("index"), RecursiveMode::NonRecursive);
    let _ = watcher.watch(&git_path.join("HEAD"), RecursiveMode::NonRecursive);
    let _ = watcher.watch(&git_path.join("refs"), RecursiveMode::Recursive);

    std::thread::spawn(move || {
        // Simple debounce: wait a bit and clear the channel of rapid events
        while let Ok(res) = rx.recv() {
            match res {
                Ok(_) => {
                    // Give Git a moment to finish its IO
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let _ = app_handle.emit("git-state-changed", ());

                    // Drain the channel of immediate subsequent events
                    while let Ok(_) = rx.try_recv() {}
                }
                Err(e) => eprintln!("watcher error: {:?}", e),
            }
        }
    });

    Some(watcher)
}

fn get_settings_path(app_handle: &tauri::AppHandle) -> std::path::PathBuf {
    let mut path = app_handle
        .path()
        .app_config_dir()
        .expect("failed to get app config dir");
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("failed to create app config dir");
    }
    path.push("settings.json");
    path
}

fn save_settings_to_disk(state: &AppState, app_handle: &tauri::AppHandle) -> Result<(), String> {
    let path = get_settings_path(app_handle);
    let json = serde_json::to_string_pretty(&state.settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_settings_from_disk(app_handle: &tauri::AppHandle) -> Settings {
    let path = get_settings_path(app_handle);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
    }
    Settings {
        user_name: String::new(),
        user_email: String::new(),
        ssh_key_path: None,
        ssh_passphrase: None,
        theme: "dark".to_string(),
        recent_repositories: Vec::new(),
        last_opened_repository: None,
    }
}

#[tauri::command]
fn open_repository(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<RepositoryInfo, String> {
    let mut state = state.0.lock().unwrap();
    match git_operations::open_repository(&path) {
        Ok(repo) => {
            let info = git_operations::get_repository_info(&repo)?;
            state.repo = Some(repo);
            state.watcher = start_watcher(app_handle.clone(), &path);
            
            // Add to recent repositories if not already there
            if !state.settings.recent_repositories.contains(&path) {
                state.settings.recent_repositories.insert(0, path.clone());
                if state.settings.recent_repositories.len() > 10 {
                    state.settings.recent_repositories.truncate(10);
                }
            }
            state.settings.last_opened_repository = Some(path);
            save_settings_to_disk(&state, &app_handle)?;
            Ok(info)
        }
        Err(e) => {
            if !std::path::Path::new(&path).exists() {
                state.settings.recent_repositories.retain(|p| p != &path);
                if state.settings.last_opened_repository == Some(path) {
                    state.settings.last_opened_repository = None;
                }
                let _ = save_settings_to_disk(&state, &app_handle);
                return Err(format!("Repository path not found. Removed from list."));
            }
            Err(e)
        }
    }
}

#[tauri::command]
fn clone_repository(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    options: CloneOptions,
) -> Result<String, String> {
    let mut state_lock = state.0.lock().unwrap();
    let repo = git_operations::clone_repository(
        &options.url,
        &options.path,
        state_lock.settings.ssh_key_path.as_deref(),
        state_lock.settings.ssh_passphrase.as_deref(),
    )?;
    let path = options.path.clone();
    state_lock.repo = Some(repo);
    state_lock.watcher = start_watcher(app_handle.clone(), &path);

    if !state_lock.settings.recent_repositories.contains(&path) {
        state_lock.settings.recent_repositories.insert(0, path.clone());
    }
    state_lock.settings.last_opened_repository = Some(path.clone());
    save_settings_to_disk(&state_lock, &app_handle)?;
    Ok(path)
}

#[tauri::command]
fn get_repository_status(state: State<'_, App>) -> Result<Vec<FileStatus>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_status(repo)
}

#[tauri::command]
fn create_commit(state: State<'_, App>, options: CommitOptions) -> Result<String, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    let stage_result = git_operations::stage_files(repo, options.files)?;
    if stage_result.staged.is_empty() && !stage_result.warnings.is_empty() {
        return Err(format!("No files could be staged: {}", stage_result.warnings.join("; ")));
    }
    git_operations::create_commit(repo, &options.message)
}

#[tauri::command]
fn stage_files(state: State<'_, App>, files: Vec<String>) -> Result<StageResult, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::stage_files(repo, files)
}

#[tauri::command]
fn unstage_files(state: State<'_, App>, files: Vec<String>) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::unstage_files(repo, files)
}

#[tauri::command]
fn discard_changes(state: State<'_, App>, file_path: String) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::discard_changes(repo, &file_path)
}

#[tauri::command]
fn get_branches(state: State<'_, App>) -> Result<Vec<BranchInfo>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_branches(repo)
}

#[tauri::command]
fn create_branch(state: State<'_, App>, options: BranchOptions) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::create_branch(repo, &options.name)
}

#[tauri::command]
fn checkout_branch(state: State<'_, App>, options: BranchOptions) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::checkout_branch(repo, &options.name)
}

#[tauri::command]
fn get_commit_diff(state: State<'_, App>, sha: String) -> Result<Vec<DiffInfo>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_commit_diff(repo, &sha)
}

#[tauri::command]
fn get_commit_history(state: State<'_, App>, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_commit_history(repo, limit)
}

#[tauri::command]
fn get_diff(state: State<'_, App>, file_path: Option<String>) -> Result<Vec<DiffInfo>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_diff(repo, file_path.as_deref())
}

#[tauri::command]
fn push_changes(state: State<'_, App>) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::push_changes(
        repo,
        state.settings.ssh_key_path.as_deref(),
        state.settings.ssh_passphrase.as_deref(),
    )
}

#[tauri::command]
fn pull_changes(state: State<'_, App>) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::pull_changes(
        repo,
        state.settings.ssh_key_path.as_deref(),
        state.settings.ssh_passphrase.as_deref(),
    )
}

#[tauri::command]
fn fetch_changes(state: State<'_, App>) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::fetch_changes(
        repo,
        state.settings.ssh_key_path.as_deref(),
        state.settings.ssh_passphrase.as_deref(),
    )
}

#[tauri::command]
fn stash_save(state: State<'_, App>, options: StashOptions) -> Result<(), String> {
    let mut state = state.0.lock().unwrap();
    let repo = state.repo.as_mut().ok_or("No repository open")?;
    git_operations::stash_save(repo, options.message.as_deref())
}

#[tauri::command]
fn stash_pop(state: State<'_, App>, index: usize) -> Result<(), String> {
    let mut state = state.0.lock().unwrap();
    let repo = state.repo.as_mut().ok_or("No repository open")?;
    git_operations::stash_pop(repo, index)
}

#[tauri::command]
fn list_stashes(state: State<'_, App>) -> Result<Vec<StashInfo>, String> {
    let mut state = state.0.lock().unwrap();
    let repo = state.repo.as_mut().ok_or("No repository open")?;
    git_operations::stash_list(repo)
}

#[tauri::command]
fn get_conflicts(state: State<'_, App>) -> Result<Vec<ConflictInfo>, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_conflicts(repo)
}

#[tauri::command]
fn resolve_conflict(state: State<'_, App>, path: String, use_ours: bool) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::resolve_conflict(repo, &path, use_ours)
}

#[tauri::command]
fn amend_commit(state: State<'_, App>, message: String) -> Result<String, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::amend_last_commit(repo, &message)
}

#[tauri::command]
fn cherry_pick(state: State<'_, App>, sha: String) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::cherry_pick(repo, &sha)
}

#[tauri::command]
fn revert_commit(state: State<'_, App>, sha: String) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::revert_commit(repo, &sha)
}

#[tauri::command]
fn discard_all_changes(state: State<'_, App>) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::discard_all_changes(repo)
}

#[tauri::command]
fn get_settings(state: State<'_, App>) -> Result<Settings, String> {
    let state = state.0.lock().unwrap();
    Ok(state.settings.clone())
}

#[tauri::command]
fn save_settings(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    settings: Settings,
) -> Result<(), String> {
    let mut state = state.0.lock().unwrap();
    state.settings = settings;
    save_settings_to_disk(&state, &app_handle)?;
    Ok(())
}

#[tauri::command]
fn set_remote_url(state: State<'_, App>, name: String, url: String) -> Result<(), String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::set_remote_url(repo, &name, &url)
}

#[tauri::command]
fn get_remote_url(state: State<'_, App>, name: String) -> Result<String, String> {
    let state = state.0.lock().unwrap();
    let repo = state.repo.as_ref().ok_or("No repository open")?;
    git_operations::get_remote_url(repo, &name)
}

#[tauri::command]
async fn get_repositories_info(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<RepositoryInfo>, String> {
    let mut results = Vec::new();
    let mut to_remove = Vec::new();

    for path in paths {
        match git_operations::open_repository(&path) {
            Ok(repo) => {
                if let Ok(info) = git_operations::get_repository_info(&repo) {
                    results.push(info);
                    continue;
                }
            }
            Err(_) => {
                if !std::path::Path::new(&path).exists() {
                    to_remove.push(path.clone());
                    continue;
                }
            }
        }
        // Fallback for valid paths that can't be opened or other errors
        results.push(RepositoryInfo {
            path,
            current_branch: "unknown".to_string(),
            is_dirty: false,
            ahead: 0,
            behind: 0,
        });
    }

    if !to_remove.is_empty() {
        let mut state = state.0.lock().unwrap();
        state.settings.recent_repositories.retain(|p| !to_remove.contains(p));
        let _ = save_settings_to_disk(&state, &app_handle);
    }

    Ok(results)
}

#[tauri::command]
fn get_current_repo_info(state: State<'_, App>) -> Result<Option<RepositoryInfo>, String> {
    let state = state.0.lock().unwrap();
    if let Some(repo) = state.repo.as_ref() {
        let info = git_operations::get_repository_info(repo)?;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let settings = load_settings_from_disk(app_handle);
            let mut repo = None;
            let mut watcher = None;
            if let Some(path) = &settings.last_opened_repository {
                if let Ok(opened_repo) = git_operations::open_repository(path) {
                    repo = Some(opened_repo);
                    watcher = start_watcher(app_handle.clone(), path);
                }
            }
            app.manage(App(Mutex::new(AppState {
                repo,
                settings,
                watcher,
            })));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_repository,
            clone_repository,
            get_repository_status,
            create_commit,
            amend_commit,
            cherry_pick,
            revert_commit,
            discard_all_changes,
            stage_files,
            unstage_files,
            discard_changes,
            get_branches,
            create_branch,
            checkout_branch,
            get_commit_diff,
            get_commit_history,
            get_diff,
            push_changes,
            pull_changes,
            fetch_changes,
            stash_save,
            stash_pop,
            list_stashes,
            get_conflicts,
            resolve_conflict,
            get_settings,
            save_settings,
            set_remote_url,
            get_remote_url,
            get_current_repo_info,
            get_repositories_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
