import type {
  ArtifactCaptureMode,
  Task,
  TaskAutomationState as PersistedTaskAutomationState,
} from "./types"
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function parseDateValue(dateStr: string): Date | null {
  const trimmed = dateStr.trim();
  if (!trimmed) {
    return null;
  }

  const normalized = trimmed.includes("T") ? trimmed : trimmed.replace(" ", "T");
  const withTimezone = /(?:Z|[+-]\d{2}:\d{2})$/i.test(normalized)
    ? normalized
    : `${normalized}Z`;
  const parsed = new Date(withTimezone);

  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

export function formatDate(dateStr: string): string {
  const parsed = parseDateValue(dateStr);
  return parsed ? parsed.toLocaleString("zh-CN") : dateStr;
}

export function getStatusColor(status: string): string {
  const colors: Record<string, string> = {
    todo: "bg-slate-500",
    in_progress: "bg-blue-500",
    review: "bg-yellow-500",
    completed: "bg-green-500",
    blocked: "bg-red-500",
    online: "bg-green-500",
    busy: "bg-yellow-500",
    offline: "bg-gray-500",
    error: "bg-red-500",
    active: "bg-green-500",
    archived: "bg-gray-500",
  };
  return colors[status] || "bg-gray-500";
}

export function getStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    todo: "待办",
    in_progress: "进行中",
    review: "审核中",
    completed: "已完成",
    blocked: "已阻塞",
    online: "在线",
    busy: "忙碌",
    offline: "离线",
    error: "错误",
    active: "活跃",
    archived: "已归档",
  };
  return labels[status] || status;
}

export function getActivityActionLabel(action: string): string {
  const labels: Record<string, string> = {
    task_created: "创建任务",
    task_status_changed: "任务状态变更",
    task_deleted: "删除任务",
    task_execution_started: "开始任务会话",
    task_execution_resumed: "继续任务会话",
    task_automation_enabled: "开启自动质控",
    task_automation_disabled: "关闭自动质控",
    task_automation_review_started: "自动质控开始审核",
    task_automation_fix_started: "自动质控开始修复",
    task_automation_commit_started: "自动质控开始提交代码",
    task_automation_commit_completed: "自动质控提交代码完成",
    task_automation_commit_failed: "自动质控提交代码失败",
    task_automation_completed: "自动质控闭环完成",
    task_automation_blocked: "自动质控阻塞",
    task_automation_manual_control: "自动质控转人工处理",
    task_automation_skip_disabled: "自动质控因配置关闭而跳过",
    task_automation_restart_requested: "重启自动质控",
    task_automation_settings_updated: "自动质控设置更新",
    git_preferences_updated: "Git 偏好设置更新",
    task_review_requested: "请求代码审核",
    task_review_started: "开始代码审核",
    task_review_completed: "代码审核完成",
    task_review_failed: "代码审核失败",
    task_worktree_enabled: "开启任务 Worktree 模式",
    task_git_context_ready: "任务代码等待自动提交",
    task_git_context_prepared: "Git 执行上下文已准备",
    task_git_context_prepare_failed: "Git 执行上下文准备失败",
    task_git_context_drift_detected: "Git 执行上下文已失效",
    task_git_context_reconciled: "Git 执行上下文已修复",
    task_git_stage_all: "暂存任务全部改动",
    task_git_committed: "提交任务代码",
    git_action_requested: "Git 高风险操作待确认",
    git_action_confirmed: "Git 高风险操作已执行",
    git_action_cancelled: "Git 高风险操作已取消",
    git_action_rejected: "Git 高风险操作已拒绝",
    task_merge_ready: "任务变更已进入待合并",
    task_worktree_cleanup_completed: "任务工作树清理完成",
    project_git_file_opened: "浏览工作区文件",
    project_git_file_previewed: "预览工作区文件",
    project_git_commit_history_viewed: "浏览项目提交历史",
    project_git_commit_detail_viewed: "查看提交详情",
    project_git_file_staged: "暂存工作区文件",
    project_git_file_unstaged: "取消暂存工作区文件",
    project_git_stage_all: "暂存全部工作区文件",
    project_git_unstage_all: "取消暂存全部工作区文件",
    project_git_committed: "创建项目提交",
    project_git_commit_message_generated: "AI 生成提交信息",
    ai_prompt_optimized: "AI 生成提示词",
    project_git_pushed: "推送项目分支",
    project_git_pulled: "拉取项目分支",
    environment_mode_switched: "切换SSH模式",
    ssh_host_selected: "切换SSH主机",
    employee_project_membership_conflict_migrated: "员工项目归属冲突已迁移",
    ssh_config_created: "新增SSH配置",
    ssh_config_updated: "更新SSH配置",
    ssh_config_deleted: "删除SSH配置",
    git_runtime_installed: "本地 Git 运行时已安装",
    git_runtime_install_failed: "本地 Git 运行时安装失败",
    remote_sdk_installed: "远程安装SDK",
    remote_git_runtime_installed: "远程 Git 运行时已安装",
    remote_git_runtime_install_failed: "远程 Git 运行时安装失败",
    remote_codex_validated: "远程校验Codex",
    remote_codex_verified: "远程校验Codex",
    remote_task_attachments_synced: "同步远程图片附件",
    remote_task_session_started: "启动远程任务会话",
    remote_session_artifact_captured: "远程会话变更明细已保存",
    remote_session_artifact_limited: "远程会话变更明细受限",
    remote_artifact_capture_limited: "远程会话变更明细受限",
    global_search_navigated: "使用全局搜索跳转",
    notification_created: "创建通知提醒",
    notification_resolved: "通知已恢复",
  };
  return labels[action] || action;
}

export function isArtifactCaptureLimited(mode: ArtifactCaptureMode): boolean {
  return mode === "ssh_git_status" || mode === "ssh_none";
}

export function getActivityDetailsLabel(action: string, details: string | null | undefined): string | null {
  if (!details) {
    return null;
  }

  if (action === "task_status_changed") {
    const separator = " -> ";
    const separatorIndex = details.lastIndexOf(separator);

    if (separatorIndex > 0) {
      const subject = details.slice(0, separatorIndex).trim();
      const nextStatus = details.slice(separatorIndex + separator.length).trim();

      if (subject && nextStatus) {
        return `${subject} -> ${getStatusLabel(nextStatus)}`;
      }
    }
  }

  return details;
}

export function getPriorityColor(priority: string): string {
  const colors: Record<string, string> = {
    low: "text-slate-500",
    medium: "text-blue-500",
    high: "text-orange-500",
    urgent: "text-red-500",
  };
  return colors[priority] || "text-gray-500";
}

export function getPriorityLabel(priority: string): string {
  const labels: Record<string, string> = {
    low: "低",
    medium: "中",
    high: "高",
    urgent: "紧急",
  };
  return labels[priority] || priority;
}

export function getEmployeeRoleLabel(role: string): string {
  const labels: Record<string, string> = {
    developer: "开发者",
    reviewer: "审查员",
    tester: "测试员",
    coordinator: "协调员",
  };

  return labels[role] || role;
}

export interface TaskAutomationDisplayState {
  enabled: boolean
  status: string
  updatedAt: string | null
  note: string | null
  source: "task" | "automation_state"
  roundCount: number | null
}

export function getTaskAutomationStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    disabled: "未开启",
    enabled: "已开启",
    idle: "待命",
    launching_review: "启动审核中",
    waiting_review: "自动审核中",
    launching_fix: "启动修复中",
    waiting_execution: "自动修复中",
    committing_code: "正在提交代码",
    review_launch_failed: "审核启动失败",
    fix_launch_failed: "修复启动失败",
    commit_failed: "提交失败",
    review_started: "自动审核中",
    fix_started: "自动修复中",
    completed: "闭环完成",
    blocked: "已阻塞",
    manual_control: "转人工处理",
    skip_disabled: "因配置关闭而跳过",
  }

  return labels[status] || status
}

const ACTIVE_REVIEW_AUTOMATION_PHASES = new Set([
  "launching_review",
  "waiting_review",
])

const ACTIVE_EXECUTION_AUTOMATION_PHASES = new Set([
  "launching_fix",
  "waiting_execution",
  "committing_code",
])

export function isTaskAutomationReviewActive(
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null,
): boolean {
  return Boolean(
    automationState?.enabled
    && ACTIVE_REVIEW_AUTOMATION_PHASES.has(automationState.status),
  )
}

export function isTaskAutomationExecutionActive(
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null,
): boolean {
  return Boolean(
    automationState?.enabled
    && ACTIVE_EXECUTION_AUTOMATION_PHASES.has(automationState.status),
  )
}

export interface TaskActionRuntimeState {
  reviewActive: boolean
  executionActive: boolean
}

export function getTaskActionRuntimeState(params: {
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null
  isReviewRunning: boolean
  isExecutionRunning: boolean
}): TaskActionRuntimeState {
  return {
    reviewActive:
      params.isReviewRunning || isTaskAutomationReviewActive(params.automationState),
    executionActive:
      params.isExecutionRunning || isTaskAutomationExecutionActive(params.automationState),
  }
}

export function getTaskAutomationDisplayState(
  task: Task,
  automationState?: PersistedTaskAutomationState | null,
): TaskAutomationDisplayState {
  const enabled = task.automation_mode === "review_fix_loop_v1"

  if (!enabled) {
    return {
      enabled: false,
      status: "disabled",
      updatedAt: task.updated_at ?? null,
      note: null,
      source: "task",
      roundCount: null,
    }
  }

  if (!automationState) {
    return {
      enabled: true,
      status: "idle",
      updatedAt: task.updated_at ?? null,
      note: null,
      source: "task",
      roundCount: 0,
    }
  }

  return {
    enabled: true,
    status: automationState.phase,
    updatedAt: automationState.updated_at ?? task.updated_at ?? null,
    note: automationState.last_error ?? automationState.last_verdict?.summary ?? null,
    source: "automation_state",
    roundCount: automationState.round_count,
  }
}
