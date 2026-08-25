// Domain slice: types (included into git_workflow)

const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

const TASK_GIT_STATE_PROVISIONING: &str = "provisioning";

const TASK_GIT_STATE_READY: &str = "ready";

const TASK_GIT_STATE_RUNNING: &str = "running";

const TASK_GIT_STATE_MERGE_READY: &str = "merge_ready";

const TASK_GIT_STATE_ACTION_PENDING: &str = "action_pending";

const TASK_GIT_STATE_COMPLETED: &str = "completed";

const TASK_GIT_STATE_FAILED: &str = "failed";

const TASK_GIT_STATE_DRIFTED: &str = "drifted";

const PENDING_ACTION_TTL_MINUTES: i64 = 15;

const PROJECT_GIT_RECENT_COMMIT_SUMMARY_LIMIT: usize = 5;

const PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_DEFAULT: usize = 20;

const PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_MAX: usize = 50;

const PROJECT_FILE_LIST_LIMIT_DEFAULT: usize = 200;

const PROJECT_FILE_LIST_LIMIT_MAX: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskGitContextRecord {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub base_branch: String,
    pub task_branch: String,
    pub target_branch: String,
    pub worktree_path: String,
    pub repo_head_commit_at_prepare: Option<String>,
    pub state: String,
    pub context_version: i64,
    pub pending_action_type: Option<String>,
    pub pending_action_token_hash: Option<String>,
    pub pending_action_payload_json: Option<String>,
    pub pending_action_nonce: Option<String>,
    pub pending_action_requested_at: Option<String>,
    pub pending_action_expires_at: Option<String>,
    pub pending_action_repo_revision: Option<String>,
    pub pending_action_bound_context_version: Option<i64>,
    pub last_reconciled_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGitContextSummary {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub base_branch: String,
    pub task_branch: String,
    pub target_branch: String,
    pub worktree_path: String,
    pub repo_head_commit_at_prepare: Option<String>,
    pub state: String,
    pub context_version: i64,
    pub pending_action_type: Option<String>,
    pub pending_action_requested_at: Option<String>,
    pub pending_action_expires_at: Option<String>,
    pub last_reconciled_at: Option<String>,
    pub last_error: Option<String>,
    pub worktree_missing: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TaskGitContextRecord> for TaskGitContextSummary {
    fn from(value: TaskGitContextRecord) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            project_id: value.project_id,
            base_branch: value.base_branch,
            task_branch: value.task_branch,
            target_branch: value.target_branch,
            worktree_path: value.worktree_path,
            repo_head_commit_at_prepare: value.repo_head_commit_at_prepare,
            state: value.state,
            context_version: value.context_version,
            pending_action_type: value.pending_action_type,
            pending_action_requested_at: value.pending_action_requested_at,
            pending_action_expires_at: value.pending_action_expires_at,
            last_reconciled_at: value.last_reconciled_at,
            last_error: value.last_error,
            worktree_missing: false,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedTaskGitExecution {
    pub task_git_context_id: String,
    pub working_dir: String,
    pub task_branch: String,
    pub target_branch: String,
    pub state: String,
    pub context_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitActionRequestResult {
    pub task_git_context_id: String,
    pub action_type: String,
    pub token: String,
    pub expires_at: String,
    pub state: String,
    pub context_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmGitActionResult {
    pub context: TaskGitContextSummary,
    pub action_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommit {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommitHistory {
    pub commits: Vec<ProjectGitCommit>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommitFileChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommitDetail {
    pub project_id: String,
    pub execution_target: String,
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub body: Option<String>,
    pub author_name: String,
    pub author_email: Option<String>,
    pub authored_at: String,
    pub diff_text: Option<String>,
    pub diff_truncated: bool,
    pub changed_files: Vec<ProjectGitCommitFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitWorkingTreeChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: String,
    pub stage_status: String,
    pub can_open_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitFilePreview {
    pub project_id: String,
    pub relative_path: String,
    pub previous_path: Option<String>,
    pub absolute_path: Option<String>,
    pub previous_absolute_path: Option<String>,
    pub execution_target: String,
    pub change_type: String,
    pub before_label: String,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: bool,
    pub after_label: String,
    pub after_status: String,
    pub after_text: Option<String>,
    pub after_truncated: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitOverview {
    pub project_id: String,
    pub repo_path: Option<String>,
    pub execution_target: String,
    pub git_runtime_provider: String,
    pub git_runtime_status: String,
    pub git_runtime_message: Option<String>,
    pub default_branch: Option<String>,
    pub current_branch: Option<String>,
    pub project_branches: Vec<String>,
    pub head_commit_sha: Option<String>,
    pub working_tree_summary: Option<String>,
    pub ahead_commits: Option<u32>,
    pub behind_commits: Option<u32>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
    pub refreshed_at: String,
    pub recent_commits: Vec<ProjectGitCommit>,
    pub recent_commits_has_more: bool,
    pub active_contexts: Vec<TaskGitContextSummary>,
    pub pending_action_contexts: Vec<TaskGitContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitWorktree {
    pub path: String,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub short_head_sha: Option<String>,
    pub is_main: bool,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
    pub is_prunable: bool,
    pub prunable_reason: Option<String>,
    pub task_git_context_id: Option<String>,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub working_tree_summary: Option<String>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGitCommitOverview {
    pub task_git_context_id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub execution_target: String,
    pub current_branch: Option<String>,
    pub working_tree_summary: Option<String>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
    pub refreshed_at: String,
}

const TASK_COMMIT_MODE_WORKTREE: &str = "worktree";
const TASK_COMMIT_MODE_PROJECT_REPO: &str = "project_repo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCommitActionState {
    pub task_id: String,
    pub project_id: String,
    pub mode: String,
    pub task_git_context_id: Option<String>,
    pub working_dir: Option<String>,
    pub execution_target: Option<String>,
    pub current_branch: Option<String>,
    pub git_context_state: Option<String>,
    pub worktree_missing: bool,
    pub has_stageable: bool,
    pub has_staged: bool,
    pub has_unmerged: bool,
    pub can_commit: bool,
    pub can_ai_commit: bool,
    pub can_merge: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCommitOverview {
    pub task_id: String,
    pub project_id: String,
    pub mode: String,
    pub task_git_context_id: Option<String>,
    pub working_dir: String,
    pub execution_target: String,
    pub current_branch: Option<String>,
    pub working_tree_summary: Option<String>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
    pub has_unmerged: bool,
    pub warning: Option<String>,
    pub refreshed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAiCommitResult {
    pub task_id: String,
    pub mode: String,
    pub message: String,
    pub detail: String,
    pub conflict_resolved: bool,
    pub merge_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAiConflictResolveResult {
    pub task_id: String,
    pub working_dir: String,
    pub resolved_files: Vec<String>,
    pub detail: String,
    pub merge_completed: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum TaskGitAutoCommitOutcome {
    Committed { detail: String },
    MergeReady { detail: String },
    NoChanges { detail: String },
}

#[derive(Debug, Clone, Default)]
struct RawWorktreeEntry {
    path: String,
    branch_ref: Option<String>,
    head_sha: Option<String>,
    is_bare: bool,
    is_detached: bool,
    is_locked: bool,
    lock_reason: Option<String>,
    is_prunable: bool,
    prunable_reason: Option<String>,
}

impl RawWorktreeEntry {
    fn branch_name(&self) -> Option<String> {
        self.branch_ref
            .as_deref()
            .and_then(normalize_git_branch_ref)
    }
}

#[derive(Debug, Clone, FromRow)]
struct TaskGitContextWorktreeRow {
    id: String,
    task_id: String,
    worktree_path: String,
    task_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestGitActionInput {
    pub task_git_context_id: String,
    pub action_type: String,
    pub payload: Value,
}

#[derive(Clone, Debug)]
struct GitProjectRuntimeContext {
    repo_path: String,
    execution_target: String,
    ssh_config_id: Option<String>,
}

impl GitProjectRuntimeContext {
    fn with_repo_path(&self, repo_path: impl Into<String>) -> Self {
        Self {
            repo_path: repo_path.into(),
            execution_target: self.execution_target.clone(),
            ssh_config_id: self.ssh_config_id.clone(),
        }
    }
}
