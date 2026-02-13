mod git_operations;
mod models;

use models::{
    BranchInfo, BranchOptions, CloneOptions, CommitInfo, CommitOptions, ConflictInfo, DiffInfo,
    FileStatus, RepositoryInfo, Settings, StageResult, StashInfo, StashOptions,
};
use notify::{Config, RecursiveMode, Watcher};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

pub enum AppError {
    Git(String),
    Io(String),
    Lock(String),
    Config(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let msg = match self {
            AppError::Git(e) => format!("Git Error: {}", e),
            AppError::Io(e) => format!("IO Error: {}", e),
            AppError::Lock(e) => format!("Concurrency Error: {}", e),
            AppError::Config(e) => format!("Config Error: {}", e),
        };
        serializer.serialize_str(&msg)
    }
}

impl From<git2::Error> for AppError {
    fn from(err: git2::Error) -> Self {
        AppError::Git(err.to_string())
    }
}

impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::Git(err)
    }
}

impl From<&str> for AppError {
    fn from(err: &str) -> Self {
        AppError::Git(err.to_string())
    }
}

struct AppState {
    repo: Option<git2::Repository>,
    settings: Settings,
    watcher: Option<notify::RecommendedWatcher>,
}

struct App(Mutex<AppState>);

type AppResult<T> = Result<T, AppError>;

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

fn get_settings_path(app_handle: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    let path = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(e.to_string()))?;
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| AppError::Io(e.to_string()))?;
    }
    Ok(path.join("settings.json"))
}

fn save_settings_to_disk(state: &AppState, app_handle: &tauri::AppHandle) -> AppResult<()> {
    let path = get_settings_path(app_handle)?;
    let json = serde_json::to_string_pretty(&state.settings).map_err(|e| AppError::Config(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

fn load_settings_from_disk(app_handle: &tauri::AppHandle) -> Settings {
    if let Ok(path) = get_settings_path(app_handle) {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
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
) -> AppResult<RepositoryInfo> {
    let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
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
                return Err(AppError::Git(format!("Repository path not found. Removed from list.")));
            }
            Err(AppError::Git(e))
        }
    }
}

#[tauri::command]
async fn clone_repository(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    options: CloneOptions,
) -> AppResult<String> {
    let (ssh_key, ssh_pass) = {
        let state_lock = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
        (state_lock.settings.ssh_key_path.clone(), state_lock.settings.ssh_passphrase.clone())
    };

    let url = options.url.clone();
    let path = options.path.clone();

    // Perform clone in a blocking thread to avoid freezing the async executor
    let repo_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        git_operations::clone_repository(
            &url,
            &repo_path,
            ssh_key.as_deref(),
            ssh_pass.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Git(format!("Spawn error: {}", e)))?
    .map_err(AppError::Git)?;

    // Re-acquire lock to update state
    let mut state_lock = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    
    // Re-open repo in state
    match git_operations::open_repository(&path) {
        Ok(repo) => {
            state_lock.repo = Some(repo);
            state_lock.watcher = start_watcher(app_handle.clone(), &path);

            if !state_lock.settings.recent_repositories.contains(&path) {
                state_lock.settings.recent_repositories.insert(0, path.clone());
            }
            state_lock.settings.last_opened_repository = Some(path.clone());
            save_settings_to_disk(&state_lock, &app_handle)?;
            Ok(path)
        }
        Err(e) => Err(AppError::Git(e)),
    }
}

#[tauri::command]
fn get_repository_status(state: State<'_, App>) -> AppResult<Vec<FileStatus>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_status(repo).map_err(AppError::Git)
}

#[tauri::command]
fn create_commit(state: State<'_, App>, options: CommitOptions) -> AppResult<String> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    let stage_result = git_operations::stage_files(repo, options.files)?;
    if stage_result.staged.is_empty() && !stage_result.warnings.is_empty() {
        return Err(AppError::Git(format!("No files could be staged: {}", stage_result.warnings.join("; "))));
    }
    git_operations::create_commit(repo, &options.message).map_err(AppError::Git)
}

#[tauri::command]
fn stage_files(state: State<'_, App>, files: Vec<String>) -> AppResult<StageResult> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::stage_files(repo, files).map_err(AppError::Git)
}

#[tauri::command]
fn unstage_files(state: State<'_, App>, files: Vec<String>) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::unstage_files(repo, files).map_err(AppError::Git)
}

#[tauri::command]
fn discard_changes(state: State<'_, App>, file_path: String) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::discard_changes(repo, &file_path).map_err(AppError::Git)
}

#[tauri::command]
fn get_branches(state: State<'_, App>) -> AppResult<Vec<BranchInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_branches(repo).map_err(AppError::Git)
}

#[tauri::command]
fn create_branch(state: State<'_, App>, options: BranchOptions) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::create_branch(repo, &options.name).map_err(AppError::Git)
}

#[tauri::command]
fn checkout_branch(state: State<'_, App>, options: BranchOptions) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::checkout_branch(repo, &options.name).map_err(AppError::Git)
}

#[tauri::command]
fn get_commit_diff(state: State<'_, App>, sha: String) -> AppResult<Vec<DiffInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_commit_diff(repo, &sha).map_err(AppError::Git)
}

#[tauri::command]
fn get_commit_history(state: State<'_, App>, limit: usize) -> AppResult<Vec<CommitInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_commit_history(repo, limit).map_err(AppError::Git)
}

#[tauri::command]
fn get_diff(state: State<'_, App>, file_path: Option<String>) -> AppResult<Vec<DiffInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_diff(repo, file_path.as_deref()).map_err(AppError::Git)
}

#[tauri::command]
async fn push_changes(state: State<'_, App>) -> AppResult<()> {
    let (path, ssh_key, ssh_pass) = {
        let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
        let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
        let path = repo.workdir().ok_or(AppError::Git("No workdir".to_string()))?.to_path_buf();
        (path, state.settings.ssh_key_path.clone(), state.settings.ssh_passphrase.clone())
    };

    tauri::async_runtime::spawn_blocking(move || {
        let repo = git_operations::open_repository(path.to_str().ok_or("Invalid path")?)?;
        git_operations::push_changes(
            &repo,
            ssh_key.as_deref(),
            ssh_pass.as_deref(),
        ).map_err(AppError::Git)
    })
    .await
    .map_err(|e| AppError::Git(format!("Spawn error: {}", e)))?
}

#[tauri::command]
async fn pull_changes(state: State<'_, App>) -> AppResult<()> {
    let (path, ssh_key, ssh_pass) = {
        let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
        let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
        let path = repo.workdir().ok_or(AppError::Git("No workdir".to_string()))?.to_path_buf();
        (path, state.settings.ssh_key_path.clone(), state.settings.ssh_passphrase.clone())
    };

    tauri::async_runtime::spawn_blocking(move || {
        let repo = git_operations::open_repository(path.to_str().ok_or("Invalid path")?)?;
        git_operations::pull_changes(
            &repo,
            ssh_key.as_deref(),
            ssh_pass.as_deref(),
        ).map_err(AppError::Git)
    })
    .await
    .map_err(|e| AppError::Git(format!("Spawn error: {}", e)))?
}

#[tauri::command]
async fn fetch_changes(state: State<'_, App>) -> AppResult<()> {
    let (path, ssh_key, ssh_pass) = {
        let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
        let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
        let path = repo.workdir().ok_or(AppError::Git("No workdir".to_string()))?.to_path_buf();
        (path, state.settings.ssh_key_path.clone(), state.settings.ssh_passphrase.clone())
    };

    tauri::async_runtime::spawn_blocking(move || {
        let repo = git_operations::open_repository(path.to_str().ok_or("Invalid path")?)?;
        git_operations::fetch_changes(
            &repo,
            ssh_key.as_deref(),
            ssh_pass.as_deref(),
        ).map_err(AppError::Git)
    })
    .await
    .map_err(|e| AppError::Git(format!("Spawn error: {}", e)))?
}

#[tauri::command]
fn stash_save(state: State<'_, App>, options: StashOptions) -> AppResult<()> {
    let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_mut().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::stash_save(repo, options.message.as_deref()).map_err(AppError::Git)
}

#[tauri::command]
fn stash_pop(state: State<'_, App>, index: usize) -> AppResult<()> {
    let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_mut().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::stash_pop(repo, index).map_err(AppError::Git)
}

#[tauri::command]
fn list_stashes(state: State<'_, App>) -> AppResult<Vec<StashInfo>> {
    let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_mut().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::stash_list(repo).map_err(AppError::Git)
}

#[tauri::command]
fn get_conflicts(state: State<'_, App>) -> AppResult<Vec<ConflictInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_conflicts(repo).map_err(AppError::Git)
}

#[tauri::command]
fn resolve_conflict(state: State<'_, App>, path: String, use_ours: bool) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::resolve_conflict(repo, &path, use_ours).map_err(AppError::Git)
}

#[tauri::command]
fn amend_commit(state: State<'_, App>, message: String) -> AppResult<String> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::amend_last_commit(repo, &message).map_err(AppError::Git)
}

#[tauri::command]
fn cherry_pick(state: State<'_, App>, sha: String) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::cherry_pick(repo, &sha).map_err(AppError::Git)
}

#[tauri::command]
fn revert_commit(state: State<'_, App>, sha: String) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::revert_commit(repo, &sha).map_err(AppError::Git)
}

#[tauri::command]
fn discard_all_changes(state: State<'_, App>) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::discard_all_changes(repo).map_err(AppError::Git)
}

#[tauri::command]
fn get_settings(state: State<'_, App>) -> AppResult<Settings> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    Ok(state.settings.clone())
}

#[tauri::command]
fn save_settings(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    settings: Settings,
) -> AppResult<()> {
    let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    state.settings = settings;
    save_settings_to_disk(&state, &app_handle)?;
    Ok(())
}

#[tauri::command]
fn set_remote_url(state: State<'_, App>, name: String, url: String) -> AppResult<()> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::set_remote_url(repo, &name, &url).map_err(AppError::Git)
}

#[tauri::command]
fn get_remote_url(state: State<'_, App>, name: String) -> AppResult<String> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    let repo = state.repo.as_ref().ok_or(AppError::Git("No repository open".to_string()))?;
    git_operations::get_remote_url(repo, &name).map_err(AppError::Git)
}

#[tauri::command]
async fn get_repositories_info(
    state: State<'_, App>,
    app_handle: tauri::AppHandle,
    paths: Vec<String>,
) -> AppResult<Vec<RepositoryInfo>> {
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
        let mut state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
        state.settings.recent_repositories.retain(|p| !to_remove.contains(p));
        let _ = save_settings_to_disk(&state, &app_handle);
    }

    Ok(results)
}

#[tauri::command]
fn get_current_repo_info(state: State<'_, App>) -> AppResult<Option<RepositoryInfo>> {
    let state = state.0.lock().map_err(|_| AppError::Lock("Failed to acquire lock".to_string()))?;
    if let Some(repo) = state.repo.as_ref() {
        let info = git_operations::get_repository_info(repo).map_err(AppError::Git)?;
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
