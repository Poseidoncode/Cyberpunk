# Task: Optimize Git GUI based on Expert Advice

## Status
- [x] Implement External State Sync (File Watcher)
- [x] Improve IPC Performance (Log Pagination/Lazy Load/Virtual Scroll)
- [x] Enhance Safety (Snapshots/Reflog)
- [x] Refine Git Engine Fallbacks
- [x] UI/UX Refinement (DAG Visualization support)

## Details

### 1. External State Sync
- [x] Add `notify` crate.
- [x] Implement a background task to watch `.git/index`, `.git/HEAD`, etc.
- [x] Emit `git-state-changed` event to frontend.

### 2. Safety Enhancements
- [x] Add `git-safety` module (in `git_operations.rs`).
- [x] Create temporary refs before destructive actions.

### 3. IPC & Performance
- [x] Verified log pagination (limit 50).
- [x] Verify diff lazy-loading (selectedFile watch).
- [x] Added `parents` to `CommitInfo` for DAG support.
