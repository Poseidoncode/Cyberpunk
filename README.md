# Cyberpunk - 跨平台桌面 Git 終端

一個現代、極簡的 Git 桌面客戶端。
基於 Tauri (Rust) + Vue3 + TypeScript，提供本地與遠端倉庫的全功能管理、視覺化 Diff 與直覺操作，輕巧安全，支援 Windows/macOS/Linux。

---

## 特色功能
- 🚀 跨平台原生效能，UI 近似終端樣式
- 📚 完整 Git 操作（clone, commit, push, pull, stash, branch, 檔案狀態等）
- 🌓 極簡暗黑主題，皆可客製化
- 🔗 一鍵配置 SSH/HTTPS 協定
- 🦀 Rust 後端強化安全/效能，前端 Vue3+TypeScript
- 📖 操作紀錄、最近專案自動保存

---

## 快速安裝與啟動

### 1. 先決條件
- Node.js (18 或以上建議)
- Rust (建議 2021 以上版本)
- Git
- Yarn 或 npm

### 2. 安裝專案依賴
```sh
npm install
# 或
yarn
```

### 3. 啟動開發模式（前後端同時 hot reload）
```sh
npm run dev
# 或
yarn dev
```

### 4. 打包正式桌面應用
```sh
npm run build
# 產生 Tauri 應用檔
```

---

## 技術架構
- **前端/UI**：Vue3, TypeScript, Tailwind CSS
- **桌面整合**：Tauri（JS/TS ↔ Rust via invoke）
- **後端**：Rust (git2 庫與自有 models, operations)
- **核心目錄**：
  - `src/` 前端主要 Vue/TS 程式
  - `src/services/git.ts` Git 操作介面（前端呼叫 Rust Command）
  - `src-tauri/` Rust 主程式
  - `src-tauri/src/models.rs` 資料結構與型別定義

---

## 操作教學 - 典型流程

1. **開啟本地倉庫**：
   - 點選 [OPEN_LOCAL] → 選擇資料夾

2. **或執行 CLONE**（複製遠端到本地）：
   - 點選 [CLONE_REMOTE] → 貼上 Git URL → 選擇資料夾

3. **主要介面快速切換**：
   - 左側 [CHANGES] 查看暫存/未暫存檔案、[HISTORY] 查看提交紀錄、[STASH] 管理堆疊快照、[CONFLICT] 處理衝突
   - 點選檔案/提交可檢視差異（支援視覺化 Diff）

4. **基本 Git 流程**：
   - 勾選/取消勾選檔案加入暫存
   - 輸入 Commit 訊息後點擊 [COMMIT]
   - 點擊 [PUSH] 推送到 remote，或 [PULL] 拉取
   - 支援 Stash, Branch（切換/建立分支），與衝突解決

5. **偏好設定與 SSH/HTTPS 切換**：
   - 點擊 [CONFIG] 可編輯 Git 用戶、郵件等
   - 可瀏覽/變更 SSH KEY 路徑或切換 HTTPS

---

## 常見問題（Troubleshooting）
- 若無法正常拉/推 remote，請確認 SSH/HTTPS 設定、網路與權限。
- 設定檔預設會自動儲存在本地（每次開啟自動載入/保存）
- Windows 下請確認 Rust 環境、GTK 及其他相依。

---

## 資安備註
關於 `glib` 依賴項的資安漏洞 (GHSA-wrw7-89jp-8q8g) 調查結果，請參閱 [SECURITY_GLIB.md](./SECURITY_GLIB.md)。

## 特別鳴謝
本專案受到不同 CLI 風格專案、GitHub Desktop、tauri-starter 等開源專案啟發與參考。

