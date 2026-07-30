import type { TaskExecutionChangeHistoryItem } from "@/lib/types";

export function getLineColor(line: string): string {
  if (line.startsWith("[ERROR]")) return "text-red-400";
  if (line.startsWith("[EXIT]")) return "text-yellow-400";
  return "text-green-400";
}

export function getAiLogColor(line: string): string {
  if (line.includes("[ERROR]")) return "text-red-400";
  if (line.includes("[WARN]")) return "text-yellow-400";
  return "text-zinc-300";
}

export function getSessionStatusLabel(statusValue: string | null | undefined) {
  switch (statusValue) {
    case "pending":
      return "准备中";
    case "running":
      return "运行中";
    case "stopping":
      return "停止中";
    case "exited":
      return "已完成";
    case "failed":
      return "失败";
    default:
      return "未开始";
  }
}

export function getExecutionChangeTypeLabel(changeType: string) {
  switch (changeType) {
    case "added":
      return "新增";
    case "modified":
      return "修改";
    case "deleted":
      return "删除";
    case "renamed":
      return "重命名";
    default:
      return changeType;
  }
}

export function getExecutionChangeTypeClassName(changeType: string) {
  switch (changeType) {
    case "added":
      return "border-emerald-500/25 bg-emerald-500/10 text-emerald-700";
    case "modified":
      return "border-blue-500/25 bg-blue-500/10 text-blue-700";
    case "deleted":
      return "border-red-500/25 bg-red-500/10 text-red-700";
    case "renamed":
      return "border-amber-500/25 bg-amber-500/10 text-amber-700";
    default:
      return "border-border bg-muted/40 text-muted-foreground";
  }
}

export function getExecutionChangeCaptureModeLabel(
  captureMode: TaskExecutionChangeHistoryItem["capture_mode"],
) {
  return captureMode === "sdk_event" ? "按 Codex 事件记录" : "按 Git 快照估算";
}

export function getExecutionChangeCaptureModeDescription(
  captureMode: TaskExecutionChangeHistoryItem["capture_mode"],
) {
  return captureMode === "sdk_event"
    ? "当前列表表示本次 Codex 会话实际改动到的文件；若可查看详情，文本快照来自会话结束时保存的记录。"
    : "当前列表基于 Git 工作区前后快照估算，可能混入其他并行任务改动；若可查看详情，文本快照来自那次估算结果保存时的预览。";
}

export function getExecutionSnapshotStatusLabel(
  status: "text" | "missing" | "binary" | "unavailable",
) {
  switch (status) {
    case "text":
      return "已保存文本快照";
    case "missing":
      return "该侧不存在文件";
    case "binary":
      return "二进制文件，未保存文本";
    case "unavailable":
      return "无法读取文本快照";
    default:
      return status;
  }
}

export function getExecutionDiffLineClassName(line: string) {
  if (line.startsWith("@@")) return "text-amber-700";
  if (line.startsWith("+") && !line.startsWith("+++")) return "text-emerald-700";
  if (line.startsWith("-") && !line.startsWith("---")) return "text-red-700";
  return "text-foreground";
}
