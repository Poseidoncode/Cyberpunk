//! 專案入口
//! 啟動 Rust backend，提供 Tauri 指令注入與組件註冊。

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    github_desktop_clone_lib::run()
}
