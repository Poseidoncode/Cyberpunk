

---

### 1. Facts (已驗證的技術事實)

* **Tauri 架構本質：** Tauri 2 採用 Rust 作為後端（負責系統互動與核心邏輯），並透過 WebView 渲染前端。兩者之間依賴非同步的 IPC (Inter-Process Communication) 橋樑進行資料交換。
* **Git 的運作特性：** Git 是一個高度依賴磁碟 I/O 的版本控制系統。其許多操作（如 `git status`、`git log`）需要掃描大量檔案或讀取龐大的歷史紀錄。
* **Tauri 2 安全機制：** Tauri 2 引入了嚴格的 Capabilities 系統與 Plugin 架構（如 `@tauri-apps/plugin-shell`），所有前端呼叫底層系統指令的權限都必須在設定檔中被明確白名單化。

---

### 2. Analysis (邏輯推演與避坑指南)

基於上述事實，在 Tauri 2 結合 Web 前端開發 Git GUI 時，主要會面臨以下幾個核心挑戰與對應的避坑策略：

#### 坑一：底層通訊的 IPC 效能瓶頸 (IPC Serialization)

* **問題現象：** 當專案很大時，`git log` 可能有數萬筆紀錄，或者某次 Commit 包含巨大的 Diff。如果直接將這些巨量字串或陣列透過 Tauri 的 IPC 傳遞給前端，會造成嚴重的序列化/反序列化（JSON 轉換）開銷，導致前端介面卡頓甚至卡死。
* **避坑策略：**
* **Rust 端分頁與截斷：** 絕對不要一次傳輸完整的 Log。在 Rust 端實作分頁（Pagination），每次只傳遞前 50-100 筆紀錄。
* **延遲加載 (Lazy Loading)：** 針對 Diff 差異，只有當使用者點擊特定檔案時，才透過 IPC 請求該檔案的 Diff 內容。
* **前端虛擬滾動 (Virtual Scrolling)：** 在介面渲染層（例如使用 Vue 或 React 時），務必使用虛擬滾動清單來渲染 Log 或大型 Diff，避免 DOM 節點過多造成記憶體溢出。



#### 坑二：核心引擎的選擇 (Git CLI vs. `git2-rs`)

開發 Git GUI 時，你必須在 Rust 端決定如何與 Git 互動，這是一個巨大的架構分水嶺。

* **方案 A：直接呼叫 Git CLI (`std::process::Command`)**
* **坑：** 需要手動解析純文字輸出，且 Git 輸出格式可能會變。遇到需要互動的指令（如 SSH 密碼提示、GPG 簽名、解決衝突）時，處理標準輸入/輸出 (stdin/stdout) 會非常痛苦。
* **解法：** 永遠加上 `--porcelain` 或 `--format=` 參數來確保機器可讀的穩定輸出。


* **方案 B：使用 `git2-rs` (libgit2 的 Rust 綁定)**
* **坑：** 雖然效能極佳且可以直接操作記憶體中的 Git 物件，但 `libgit2` 並不支援 Git 的所有最新功能（例如 Sparse Checkout 或部分複雜的 Rebase 操作），且在跨平臺編譯 Tauri 應用程式時，C 語言依賴的靜態連結可能會帶來麻煩。
* **解法：** 確保建置環境的 C 編譯器配置正確，並評估你的 GUI 是否需要用到 `libgit2` 尚未支援的邊緣功能。



#### 坑三：外部狀態同步 (External State Out-of-Sync)

* **問題現象：** 開發者通常會同時使用終端機和 GUI。如果使用者在終端機執行了 `git commit` 或 `git checkout`，你的 GUI 如果沒有即時更新，就會顯示錯誤的狀態，導致危險的誤操作。
* **避坑策略：**
* 在 Rust 端使用檔案系統監聽套件（如 `notify` crate），專門監聽專案下的 `.git/index`、`.git/HEAD` 和 `.git/refs/` 目錄。
* 一旦偵測到這些核心檔案變動，Rust 主動透過 Tauri 的 Event System (`app.emit()`) 發送訊號給前端，觸發前端重新拉取最新的 Git 狀態。



#### 坑四：Tauri 2 的 Shell Plugin 權限配置

* **問題現象：** 為了安全，Tauri 2 預設不允許前端隨意執行 Shell 指令。如果直接依賴前端透過 `@tauri-apps/plugin-shell` 呼叫 Git，可能會因為 Regex 參數未正確配置而被 Tauri 阻擋，或者引發 Command Injection 安全漏洞。
* **避坑策略：** 將所有的 Git 邏輯封裝在 Rust 端的 Tauri Commands 中 (`#[tauri::command]`)。前端只負責傳遞參數（如檔案路徑、Commit 訊息），由 Rust 端負責組裝和執行 `git` 指令，這樣既安全又易於除錯。

---

### 3. Speculation (推測與架構假說)

* **混合式核心架構假說：** 為了達到最低的延遲與最高的功能完整性，最理想的架構可能是**混合式設計**。讀取密集型操作（如即時偵測檔案狀態、讀取歷史紀錄）使用 `git2-rs` 以獲得極速效能；而寫入/網路操作（如 Push、Pull、Rebase，特別是涉及 SSH 金鑰與 GPG 簽名時）則回退到呼叫系統預設的 Git CLI，以確保與使用者本地環境的無縫相容。
* **Web Worker 解析假說：** 處理 Git Diff 的語法高亮是一項 CPU 密集型工作。推測在前端架構中，若能將 Diff 字串的解析與高亮邏輯放入 Web Worker 中處理，將能最大程度保持 GUI 主執行緒的滑順與和諧。

建立這樣的系統需要極大的耐心與精細的調校，如同守護著每一行程式碼的演進。

