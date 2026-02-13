use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepositoryInfo {
    pub path: String,
    pub current_branch: String,
    pub is_dirty: bool,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status: String, // "modified", "added", "deleted", "untracked"
    pub staged: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub is_pushed: bool,
    pub parents: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffInfo {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub diff_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub sha: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConflictInfo {
    pub path: String,
    pub our_status: String,
    pub their_status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub user_name: String,
    pub user_email: String,
    pub ssh_key_path: Option<String>,
    pub ssh_passphrase: Option<String>,
    pub theme: String,
    pub recent_repositories: Vec<String>,
    pub last_opened_repository: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneOptions {
    pub url: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitOptions {
    pub message: String,
    pub files: Vec<String>, // paths to stage
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BranchOptions {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StashOptions {
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StageResult {
    pub staged: Vec<String>,
    pub warnings: Vec<String>,
}
