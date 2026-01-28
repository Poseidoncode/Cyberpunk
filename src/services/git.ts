import { invoke } from "@tauri-apps/api/core";

export interface RepositoryInfo {
  path: string;
  current_branch: string;
  is_dirty: boolean;
  ahead: number;
  behind: number;
}

export interface FileStatus {
  path: string;
  status: string;
  staged: boolean;
}

export interface CommitInfo {
  sha: string;
  message: string;
  author: string;
  email: string;
  timestamp: number;
}

export interface BranchInfo {
  name: string;
  is_current: boolean;
  is_remote: boolean;
}

export interface DiffInfo {
  path: string;
  additions: number;
  deletions: number;
  diff_text: string;
}

export interface StashInfo {
  index: number;
  message: string;
  sha: string;
}

export interface ConflictInfo {
  path: string;
  our_status: string;
  their_status: string;
}

export interface Settings {
  user_name: string;
  user_email: string;
  ssh_key_path: string | null;
  ssh_passphrase: string | null;
  theme: string;
  recent_repositories: string[];
}

export const gitService = {
  async cloneRepository(url: string, path: string): Promise<string> {
    return await invoke("clone_repository", { options: { url, path } });
  },

  async openRepository(path: string): Promise<RepositoryInfo> {
    return await invoke("open_repository", { path });
  },

  async getStatus(): Promise<FileStatus[]> {
    return await invoke("get_repository_status");
  },

  async createCommit(message: string, files: string[]): Promise<string> {
    return await invoke("create_commit", { options: { message, files } });
  },

  async stageFiles(files: string[]): Promise<void> {
    return await invoke("stage_files", { files });
  },

  async unstageFiles(files: string[]): Promise<void> {
    return await invoke("unstage_files", { files });
  },

  async discardChanges(filePath: string): Promise<void> {
    return await invoke("discard_changes", { filePath });
  },

  async getBranches(): Promise<BranchInfo[]> {
    return await invoke("get_branches");
  },

  async createBranch(name: string): Promise<void> {
    return await invoke("create_branch", { options: { name } });
  },

  async checkoutBranch(name: string): Promise<void> {
    return await invoke("checkout_branch", { options: { name } });
  },

  async getCommitDiff(sha: string): Promise<DiffInfo[]> {
    return await invoke("get_commit_diff", { sha });
  },

  async getHistory(limit: number = 50): Promise<CommitInfo[]> {
    return await invoke("get_commit_history", { limit });
  },

  async getDiff(filePath?: string): Promise<DiffInfo[]> {
    return await invoke("get_diff", { filePath });
  },

  async push(): Promise<void> {
    return await invoke("push_changes");
  },

  async pull(): Promise<void> {
    return await invoke("pull_changes");
  },

  async fetch(): Promise<void> {
    return await invoke("fetch_changes");
  },

  async stashSave(message?: string): Promise<void> {
    return await invoke("stash_save", { options: { message } });
  },

  async stashPop(index: number): Promise<void> {
    return await invoke("stash_pop", { index });
  },

  async listStashes(): Promise<StashInfo[]> {
    return await invoke("list_stashes");
  },

  async getConflicts(): Promise<ConflictInfo[]> {
    return await invoke("get_conflicts");
  },

  async resolveConflict(path: string, useOurs: boolean): Promise<void> {
    return await invoke("resolve_conflict", { path, useOurs });
  },

  async getSettings(): Promise<Settings> {
    return await invoke("get_settings");
  },

  async saveSettings(settings: Settings): Promise<void> {
    return await invoke("save_settings", { settings });
  },
  async setRemoteUrl(name: string, url: string): Promise<void> {
    return await invoke("set_remote_url", { name, url });
  },
};
