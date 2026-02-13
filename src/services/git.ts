import { invoke } from "@tauri-apps/api/core";

/**
 * 儲存當前 repository 的基本資訊
 */
export interface RepositoryInfo {
  /** 倉庫本機路徑 */
  path: string;
  /** 目前分支 */
  current_branch: string;
  /** 是否有未提交更動 */
  is_dirty: boolean;
  /** 本地 ahead remote 幾個 commit */
  ahead: number;
  /** 本地 behind remote 幾個 commit */
  behind: number;
}

/**
 * 個別檔案狀態
 */
export interface FileStatus {
  path: string;
  status: string;
  staged: boolean;
}

/**
 * 提交資訊
 */
export interface CommitInfo {
  sha: string;
  message: string;
  author: string;
  email: string;
  timestamp: number; // epoch 秒
  is_pushed: boolean;
  parents: string[];
}

/**
 * 分支資訊
 */
export interface BranchInfo {
  name: string;
  is_current: boolean;
  is_remote: boolean;
}

/**
 * 某檔案/提交的差異（diff）
 */
export interface DiffInfo {
  path: string;
  additions: number;
  deletions: number;
  diff_text: string;
}

/**
 * Stash 資訊
 */
export interface StashInfo {
  index: number;
  message: string;
  sha: string;
}

/**
 * 衝突資訊，用於 conflict 解決
 */
export interface ConflictInfo {
  path: string;
  our_status: string;
  their_status: string;
}

/**
 * Git 操作的本地設定資料
 */
export interface Settings {
  user_name: string;
  user_email: string;
  ssh_key_path: string | null;
  ssh_passphrase: string | null;
  theme: string;
  recent_repositories: string[];
  last_opened_repository: string | null;
}

/**
 * Stage 操作結果，支援部分成功
 */
export interface StageResult {
  staged: string[];
  warnings: string[];
}

/**
 * 提供所有 Git 前端操作的方法介面，實際會呼叫 Rust 後端 command
 */
export const gitService = {
  /**
   * 從遠端 Clone 倉庫
   * @param url Git 遠端 clone url
   * @param path 本機儲存路徑
   * @returns clone 後 repo 路徑
   */
  async cloneRepository(url: string, path: string): Promise<string> {
    return await invoke("clone_repository", { options: { url, path } });
  },

  /**
   * 開啟本地倉庫
   * @param path local repo 資料夾
   */
  async openRepository(path: string): Promise<RepositoryInfo> {
    return await invoke("open_repository", { path });
  },

  /**
   * 查詢目前所有檔案狀態
   */
  async getStatus(): Promise<FileStatus[]> {
    return await invoke("get_repository_status");
  },

  /**
   * 建立 commit (並同時能 stage 多檔)
   * @param message commit 訊息
   * @param files 要加入 commit 的檔案
   * @returns commit SHA
   */
  async createCommit(message: string, files: string[]): Promise<string> {
    return await invoke("create_commit", { options: { message, files } });
  },

  /**
   * 修正最後一次 commit
   */
  async amendCommit(message: string): Promise<string> {
    return await invoke("amend_commit", { message });
  },

  /**
   * 挑選特定 commit 併入當前分支
   */
  async cherryPick(sha: string): Promise<void> {
    return await invoke("cherry_pick", { sha });
  },

  /**
   * 反轉特定 commit
   */
  async revertCommit(sha: string): Promise<void> {
    return await invoke("revert_commit", { sha });
  },

  /**
   * 將多個檔案加入暫存
   */
  async stageFiles(files: string[]): Promise<StageResult> {
    return await invoke("stage_files", { files });
  },
  /**
   * 將多個檔案從暫存移除
   */
  async unstageFiles(files: string[]): Promise<void> {
    return await invoke("unstage_files", { files });
  },

  /**
   * 丟棄單一檔案的所有變動
   */
  async discardChanges(filePath: string): Promise<void> {
    return await invoke("discard_changes", { filePath });
  },

  /**
   * 丟棄所有未提交的變動 (一鍵還原)
   */
  async discardAllChanges(): Promise<void> {
    return await invoke("discard_all_changes");
  },

  /**
   * 查詢所有分支
   */
  async getBranches(): Promise<BranchInfo[]> {
    return await invoke("get_branches");
  },

  /**
   * 建立新分支
   * @param name 分支名稱
   */
  async createBranch(name: string): Promise<void> {
    return await invoke("create_branch", { options: { name } });
  },

  /**
   * 切換分支
   * @param name 分支名稱
   */
  async checkoutBranch(name: string): Promise<void> {
    return await invoke("checkout_branch", { options: { name } });
  },

  /**
   * 查詢某次提交的差異（完整diff）
   * @param sha commit SHA
   */
  async getCommitDiff(sha: string): Promise<DiffInfo[]> {
    return await invoke("get_commit_diff", { sha });
  },

  /**
   * 取得提交紀錄
   * @param limit 限制最大數量（預設50）
   */
  async getHistory(limit: number = 50): Promise<CommitInfo[]> {
    return await invoke("get_commit_history", { limit });
  },

  /**
   * 取得當前 (或特定檔案) diff
   * @param filePath 檔案路徑（可不填）
   */
  async getDiff(filePath?: string): Promise<DiffInfo[]> {
    return await invoke("get_diff", { filePath });
  },

  /**
   * push 變動至遠端
   */
  async push(): Promise<void> {
    return await invoke("push_changes");
  },

  /**
   * 從遠端 pull 變更
   */
  async pull(): Promise<void> {
    return await invoke("pull_changes");
  },

  /**
   * fetch 遠端資料但不合併
   */
  async fetch(): Promise<void> {
    return await invoke("fetch_changes");
  },

  /**
   * 存放當前變更至 stash
   * @param message 可選，stash 訊息
   */
  async stashSave(message?: string): Promise<void> {
    return await invoke("stash_save", { options: { message } });
  },

  /**
   * 還原特定 stash
   * @param index stack 序號
   */
  async stashPop(index: number): Promise<void> {
    return await invoke("stash_pop", { index });
  },

  /**
   * 查詢目前所有 stash
   */
  async listStashes(): Promise<StashInfo[]> {
    return await invoke("list_stashes");
  },

  /**
   * 查詢當前所有衝突檔案
   */
  async getConflicts(): Promise<ConflictInfo[]> {
    return await invoke("get_conflicts");
  },

  /**
   * 解決指定衝突
   * @param path 衝突檔案路徑
   * @param useOurs true:用 ours，false:theirs
   */
  async resolveConflict(path: string, useOurs: boolean): Promise<void> {
    return await invoke("resolve_conflict", { path, useOurs });
  },

  /**
   * 讀取偏好/設定
   */
  async getSettings(): Promise<Settings> {
    return await invoke("get_settings");
  },

  /**
   * 儲存偏好/設定
   */
  async saveSettings(settings: Settings): Promise<void> {
    return await invoke("save_settings", { settings });
  },

  /**
   * 變更遠端 url
   */
  async setRemoteUrl(name: string, url: string): Promise<void> {
    return await invoke("set_remote_url", { name, url });
  },

  /**
   * 取得遠端 url
   */
  /**
   * 取得目前已開啟的倉庫資訊（若有）
   */
  async getCurrentRepoInfo(): Promise<RepositoryInfo | null> {
    return await invoke("get_current_repo_info");
  },
  /**
   * 取得多個倉庫的資訊
   */
  async getRepositoriesInfo(paths: string[]): Promise<RepositoryInfo[]> {
    return await invoke("get_repositories_info", { paths });
  },
};

