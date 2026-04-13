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
