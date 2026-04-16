import type { Task, TaskAutomationState as PersistedTaskAutomationState } from "./types"
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatDate(dateStr: string): string {
  // SQLite datetime('now') returns UTC without timezone suffix (e.g. "2025-04-12 06:30:00").
  // Append "Z" so JS Date parses it as UTC, then toLocaleString converts to local timezone.
  const utc = dateStr.endsWith("Z") ? dateStr : dateStr + "Z";
  return new Date(utc).toLocaleString("zh-CN");
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
    task_automation_completed: "自动质控闭环完成",
    task_automation_blocked: "自动质控阻塞",
    task_automation_manual_control: "自动质控转人工处理",
    task_automation_skip_disabled: "自动质控因配置关闭而跳过",
    task_automation_settings_updated: "自动质控设置更新",
    task_review_requested: "请求代码审核",
    task_review_started: "开始代码审核",
    task_review_completed: "代码审核完成",
    task_review_failed: "代码审核失败",
  };
  return labels[action] || action;
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
    review_launch_failed: "审核启动失败",
    fix_launch_failed: "修复启动失败",
    review_started: "自动审核中",
    fix_started: "自动修复中",
    completed: "闭环完成",
    blocked: "已阻塞",
    manual_control: "转人工处理",
    skip_disabled: "因配置关闭而跳过",
  }

  return labels[status] || status
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
