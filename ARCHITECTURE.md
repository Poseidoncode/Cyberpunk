# Cyberpunk 架構說明

## 技術堆疊
- **桌面容器**  ： Tauri (Rust)
- **前端 UI**    ： Vue 3 + TypeScript + Tailwind
- **後端邏輯**   ： Rust (git2 操作, serde, Tauri commands)

## 核心元件

```
┌────────────┐       Tauri invoke        ┌───────────┐
│  Vue3 UI   │ ────────────────────────▶│   Rust    │
│ (TypeScript│<──────────────────────── │(Command   │
│   + Pinia) │    結果 (Promise)        │ handlers) │
└────────────┘                          └───────────┘
```

- Vue3 端事件、按鈕 → 對應 gitService 方法 (下發指令到 Rust 後端)
- Rust 端只處理「實際 git 操作」與安全檔案、設定存取
- 全部資料透過 Tauri invoke (即 async Promise)
- 設定寫在 settings.json，開啟/clone 時主動填 recent_repositories

## 前端主要檔案
- `src/App.vue`：全畫面控制、UI狀態流管理
- `src/services/git.ts`：所有 git 呼叫方法（直接對應 tauri Rust Command 名稱）

## Rust 主要檔案
- `src-tauri/src/main.rs`：Tauri app 啟動、命令註冊
- `src-tauri/src/lib.rs` ：命令實作流、全域狀態（路徑、設定）管理
- `src-tauri/src/models.rs`：主要資料結構，如 CommitInfo, BranchInfo, Settings
- `src-tauri/src/git_operations.rs` ：各類 git 底層功能實作

## 資料流舉例

1. Vue 點擊 [COMMIT] →
2. 調 gitService.createCommit →
3. Tauri invoke Rust command `create_commit` (lib.rs) →
4. Rust 執行 git2 實際 commit 操作 →
5. 結果回傳前端 → UI/歷史同步

## 設定讀寫流程
- 進 app 時，main.rs setup/init -> 用 app config 路徑自動讀取/寫入 settings.json

## 擴充策略
- 每增功能，只需同步寫一個 gitService 方法 + Rust command handler
- 資料定義由 models.rs 雙方對應（目前型別已全面導出給前端使用）

---
