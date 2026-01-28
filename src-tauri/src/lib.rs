mod models;
mod git_operations;

use models::{BranchOptions, CloneOptions, CommitOptions, StashOptions, Settings};
use std::sync::Mutex;
use tauri::{State, Manager};

struct AppState {
    current_repo_path: Mutex<Option<String>>,
    settings: Mutex<Settings>,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

fn get_config_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let path = app_handle.path().app_config_dir().map_err(|e| e.to_string())?;
    
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }
    
    Ok(path.join("settings.json"))
}

fn save_settings_internal(settings: &Settings, app_handle: &tauri::AppHandle) -> Result<(), String> {
    let path = get_config_path(app_handle)?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<AppState>, app_handle: tauri::AppHandle) -> Result<(), String> {
    *state.settings.lock().unwrap() = settings.clone();
    save_settings_internal(&settings, &app_handle)
}

#[tauri::command]
fn clone_repository(options: CloneOptions, state: State<AppState>, app_handle: tauri::AppHandle) -> Result<String, String> {
    let repo = git_operations::clone_repository(&options.url, &options.path, state.settings.lock().unwrap().ssh_key_path.as_deref())?;
    let repo_path = repo.path().to_string_lossy().to_string();
    
    *state.current_repo_path.lock().unwrap() = Some(options.path.clone());
    
    let mut settings = state.settings.lock().unwrap();
    if !settings.recent_repositories.contains(&options.path) {
        settings.recent_repositories.push(options.path.clone());
        let _ = save_settings_internal(&settings, &app_handle);
    }

    Ok(repo_path)
}

#[tauri::command]
fn open_repository(path: String, state: State<AppState>, app_handle: tauri::AppHandle) -> Result<models::RepositoryInfo, String> {
    let repo = git_operations::open_repository(&path)?;
    let info = git_operations::get_repository_info(&repo)?;
    
    *state.current_repo_path.lock().unwrap() = Some(path.clone());
    
    let mut settings = state.settings.lock().unwrap();
    if !settings.recent_repositories.contains(&path) {
        settings.recent_repositories.push(path);
        let _ = save_settings_internal(&settings, &app_handle);
    }

    Ok(info)
}

#[tauri::command]
fn get_repository_status(state: State<AppState>) -> Result<Vec<models::FileStatus>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::get_status(&repo)
}

#[tauri::command]
fn create_commit(options: CommitOptions, state: State<AppState>) -> Result<String, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    
    // Stage files
    git_operations::stage_files(&repo, options.files)?;
    
    // Create commit
    git_operations::create_commit(&repo, &options.message)
}

#[tauri::command]
fn get_branches(state: State<AppState>) -> Result<Vec<models::BranchInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::get_branches(&repo)
}

#[tauri::command]
fn create_branch(options: BranchOptions, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::create_branch(&repo, &options.name)
}

#[tauri::command]
fn checkout_branch(options: BranchOptions, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::checkout_branch(&repo, &options.name)
}

#[tauri::command]
fn get_commit_history(limit: usize, state: State<AppState>) -> Result<Vec<models::CommitInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::get_commit_history(&repo, limit)
}

#[tauri::command]
fn get_diff(file_path: Option<String>, state: State<AppState>) -> Result<Vec<models::DiffInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::get_diff(&repo, file_path.as_deref())
}
#[tauri::command]
fn get_commit_diff(sha: String, state: State<AppState>) -> Result<Vec<models::DiffInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::get_commit_diff(&repo, &sha)
}

#[tauri::command]
fn push_changes(state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::push_changes(&repo, state.settings.lock().unwrap().ssh_key_path.as_deref())
}

#[tauri::command]
fn pull_changes(state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::pull_changes(&repo, state.settings.lock().unwrap().ssh_key_path.as_deref())
}

#[tauri::command]
fn fetch_changes(state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::fetch_changes(&repo, state.settings.lock().unwrap().ssh_key_path.as_deref())
}

#[tauri::command]
fn stage_files(files: Vec<String>, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::stage_files(&repo, files)
}

#[tauri::command]
fn unstage_files(files: Vec<String>, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::unstage_files(&repo, files)
}

#[tauri::command]
fn discard_changes(file_path: String, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    
    let repo = git_operations::open_repository(path)?;
    git_operations::discard_changes(&repo, &file_path)
}

#[tauri::command]
fn stash_save(options: StashOptions, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    let mut repo = git_operations::open_repository(path)?;
    git_operations::stash_save(&mut repo, options.message.as_deref())
}

#[tauri::command]
fn stash_pop(index: usize, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    let mut repo = git_operations::open_repository(path)?;
    git_operations::stash_pop(&mut repo, index)
}

#[tauri::command]
fn list_stashes(state: State<AppState>) -> Result<Vec<models::StashInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    let mut repo = git_operations::open_repository(path)?;
    git_operations::stash_list(&mut repo)
}

#[tauri::command]
fn get_conflicts(state: State<AppState>) -> Result<Vec<models::ConflictInfo>, String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    let repo = git_operations::open_repository(path)?;
    git_operations::get_conflicts(&repo)
}

#[tauri::command]
fn resolve_conflict(path: String, use_ours: bool, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let repo_path_val = repo_path.as_ref().ok_or("No repository open")?;
    let repo = git_operations::open_repository(repo_path_val)?;
    git_operations::resolve_conflict(&repo, &path, use_ours)
}

#[tauri::command]
fn set_remote_url(name: String, url: String, state: State<AppState>) -> Result<(), String> {
    let repo_path = state.current_repo_path.lock().unwrap();
    let path = repo_path.as_ref().ok_or("No repository open")?;
    let repo = git_operations::open_repository(path)?;
    git_operations::set_remote_url(&repo, &name, &url)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle();
            let config_path = get_config_path(app_handle).unwrap_or_else(|_| std::path::PathBuf::from("settings.json"));
            
            let initial_settings = if config_path.exists() {
                let content = std::fs::read_to_string(config_path).unwrap_or_default();
                serde_json::from_str(&content).unwrap_or_else(|_| Settings {
                    user_name: "User".to_string(),
                    user_email: "user@example.com".to_string(),
                    ssh_key_path: None,
                    theme: "dark".to_string(),
                    recent_repositories: Vec::new(),
                })
            } else {
                Settings {
                    user_name: "User".to_string(),
                    user_email: "user@example.com".to_string(),
                    ssh_key_path: None,
                    theme: "dark".to_string(),
                    recent_repositories: Vec::new(),
                }
            };

            app.manage(AppState {
                current_repo_path: Mutex::new(None),
                settings: Mutex::new(initial_settings),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            clone_repository,
            open_repository,
            get_repository_status,
            create_commit,
            get_branches,
            create_branch,
            checkout_branch,
            get_commit_history,
            get_commit_diff,
            get_diff,
            push_changes,
            pull_changes,
            fetch_changes,
            stage_files,
            unstage_files,
            discard_changes,
            stash_save,
            stash_pop,
            list_stashes,
            get_conflicts,
            resolve_conflict,
            get_settings,
            save_settings,
            set_remote_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
