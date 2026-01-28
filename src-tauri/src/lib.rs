//! 主要 Tauri Rust 後端邏輯（命令註冊 & 狀態管理）
// 詳細命令「如何呼叫」、「路由流向」參閱下方 #[tauri::command] 各函式
// 共用資料型別於 models.rs 定義

mod git_operations;
mod models;

use models::{BranchOptions, CloneOptions, CommitOptions, Settings, StashOptions};
use std::sync::Mutex;
use tauri::{Manager, State};

// 其餘內容維持原樣
// ...
