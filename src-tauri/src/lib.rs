//! 主要 Tauri Rust 後端邏輯（命令註冊 & 狀態管理）
// 詳細命令「如何呼叫」、「路由流向」參閱下方 #[tauri::command] 各函式
// 共用資料型別於 models.rs 定義

mod git_operations;
mod models;

use models::{BranchOptions, CloneOptions, CommitOptions, Settings, StashOptions};
use std::sync::Mutex;
use tauri::{Manager, State};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
