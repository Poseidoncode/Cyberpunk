# Glib 資安漏洞調查紀錄 (GHSA-wrw7-89jp-8q8g)

## 0. 快速結論
- **目前影響**：**無**（因為你目前在 macOS 開發，此漏洞僅影響 Linux 平台）。
- **未來風險**：若未來要發布 **Linux** 版本，此漏洞會存在於依賴鏈中，需等待 Tauri 官方升級底層依賴。

---

## 1. 漏洞詳情
- **漏洞編號**：[GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)
- **受影響組件**：`glib` (Rust bindings)
- **受影響版本**：`>= 0.15.0, < 0.20.0`
- **問題描述**：`VariantStrIter` 的實現存在記憶體安全問題（Unsoundness），可能導致應用程式崩潰。

---

## 2. 為什麼現在修不掉？
這個問題出在 **Tauri 的上游依賴鏈**：
1. Tauri (Linux) 依賴 `webkit2gtk`。
2. `webkit2gtk` 依賴 `gtk3-rs`。
3. `gtk3-rs` 目前鎖定在 `glib 0.18.5`。
4. **關鍵點**：`gtk3-rs` 已經停止維護，無法直接升級到已修復的 `glib 0.20`。

這是一個結構性問題，需要等 Tauri 官方遷移到 `gtk4-rs` 或提供其他 Linux 渲染方案。

---

## 3. 平台的風險評估
- **macOS (目前)**：✅ **安全**。macOS 使用系統原生 WKWebView，完全不使用 glib 相關套件。
- **Windows**：✅ **安全**。Windows 使用 WebView2，不涉及此依賴。
- **Linux (未來)**：⚠️ **有風險**。若要支援 Linux，需關注 Tauri 官方的修復進度。

---

## 4. 追蹤建議
若未來考慮支援 Linux，請檢查以下 Issue 的進度：
- [Tauri Issue #12048](https://github.com/tauri-apps/tauri/issues/12048)
- [Tauri PR #12098](https://github.com/tauri-apps/tauri/pull/12098)

---
*紀錄時間：2026-02-02*
