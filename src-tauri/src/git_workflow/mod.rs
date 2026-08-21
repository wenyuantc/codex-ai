use std::collections::{hash_map::DefaultHasher, HashMap};
#[cfg(test)]
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{FromRow, SqlitePool};
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::app::{
    fetch_project_by_id, fetch_task_by_id, insert_activity_log, now_sqlite, sqlite_pool,
    EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH, PROJECT_TYPE_SSH,
};
use crate::codex::{
    generate_commit_message_for_project, load_codex_settings, load_remote_codex_settings,
};
use crate::db::models::{GitPreferences, Project, Task};
use crate::git_runtime::{
    self, GIT_RUNTIME_PROVIDER_SIMPLE_GIT, GIT_RUNTIME_STATUS_READY, GIT_RUNTIME_STATUS_UNAVAILABLE,
};
use crate::task_automation;

// File-split for navigation; items remain in this module namespace via include!.
include!("types.rs");
include!("runtime.rs");
include!("worktree.rs");
include!("context.rs");
include!("project_ops.rs");
include!("branch.rs");
include!("pending_action.rs");
include!("task_commit.rs");

#[cfg(test)]
mod tests;
