import i18n from "@/lib/i18n";
import { mapRuntimeStatusMessage } from "@/lib/i18n/mapRuntimeStatusMessage";
import type { TaskExecutionChangeHistoryItem } from "@/lib/types";

const USER_INPUT_TAG = "[USER_INPUT]";
const END_SESSION_TAG = "[END_SESSION]";
const ERROR_TAG = "[ERROR]";

/** Localize stable mid-session terminal tags emitted by Rust. */
export function formatTerminalLine(line: string): string {
  if (line.startsWith(`${ERROR_TAG} `)) {
    return `${ERROR_TAG} ${mapRuntimeStatusMessage(line.slice(ERROR_TAG.length + 1))}`;
  }
  if (line === ERROR_TAG) {
    return ERROR_TAG;
  }
  if (line.startsWith(`${USER_INPUT_TAG} `)) {
    return `${i18n.t("sessions:terminalUserInputPrefix")} ${line.slice(USER_INPUT_TAG.length + 1)}`;
  }
  if (line.startsWith(USER_INPUT_TAG)) {
    return i18n.t("sessions:terminalUserInputPrefix");
  }
  if (line.startsWith(END_SESSION_TAG)) {
    return i18n.t("sessions:terminalEndSession");
  }
  // Legacy Chinese tags from earlier builds
  if (line.startsWith("[用户输入] ")) {
    return `${i18n.t("sessions:terminalUserInputPrefix")} ${line.slice("[用户输入] ".length)}`;
  }
  if (line.startsWith("[结束会话]")) {
    return i18n.t("sessions:terminalEndSession");
  }
  if (line.startsWith("[PERMISSION] ")) {
    return `[PERMISSION] ${mapRuntimeStatusMessage(line.slice("[PERMISSION] ".length))}`;
  }
  return mapRuntimeStatusMessage(line);
}

export function getLineColor(line: string): string {
  if (line.startsWith("[ERROR]")) return "text-red-400";
  if (line.startsWith("[EXIT]")) return "text-yellow-400";
  if (
    line.startsWith(USER_INPUT_TAG) ||
    line.startsWith(END_SESSION_TAG) ||
    line.startsWith("[用户输入]") ||
    line.startsWith("[结束会话]")
  ) {
    return "text-sky-300";
  }
  if (line.startsWith("[思考]")) return "text-zinc-500";
  if (
    line.startsWith("[命令]") ||
    line.startsWith("[工具]") ||
    line.startsWith("[读取]") ||
    line.startsWith("[写入]") ||
    line.startsWith("[编辑]") ||
    line.startsWith("[补丁]") ||
    line.startsWith("[技能]") ||
    line.startsWith("[工具结果]")
  ) {
    return "text-cyan-400";
  }
  if (line.startsWith("[子 Agent]")) return "text-teal-400";
  if (line.startsWith("[计划]") || line.startsWith("[PLAN]") || line.startsWith("[待办]")) {
    return "text-violet-400";
  }
  if (line.startsWith("[用量]")) return "text-zinc-500";
  if (line.startsWith("[STDERR]")) return "text-orange-400";
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
      return i18n.t("tasks:detail.labels.sessionStatus.pending");
    case "running":
      return i18n.t("tasks:detail.labels.sessionStatus.running");
    case "stopping":
      return i18n.t("tasks:detail.labels.sessionStatus.stopping");
    case "exited":
      return i18n.t("tasks:detail.labels.sessionStatus.exited");
    case "failed":
      return i18n.t("tasks:detail.labels.sessionStatus.failed");
    default:
      return i18n.t("tasks:detail.labels.sessionStatus.default");
  }
}

export function getExecutionChangeTypeLabel(changeType: string) {
  switch (changeType) {
    case "added":
      return i18n.t("tasks:detail.labels.changeType.added");
    case "modified":
      return i18n.t("tasks:detail.labels.changeType.modified");
    case "deleted":
      return i18n.t("tasks:detail.labels.changeType.deleted");
    case "renamed":
      return i18n.t("tasks:detail.labels.changeType.renamed");
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
  return captureMode === "sdk_event"
    ? i18n.t("tasks:detail.labels.captureMode.sdkEvent")
    : i18n.t("tasks:detail.labels.captureMode.gitSnapshot");
}

export function getExecutionChangeCaptureModeDescription(
  captureMode: TaskExecutionChangeHistoryItem["capture_mode"],
) {
  return captureMode === "sdk_event"
    ? i18n.t("tasks:detail.labels.captureModeDesc.sdkEvent")
    : i18n.t("tasks:detail.labels.captureModeDesc.gitSnapshot");
}

export function getExecutionSnapshotStatusLabel(
  status: "text" | "missing" | "binary" | "unavailable",
) {
  switch (status) {
    case "text":
      return i18n.t("tasks:detail.labels.snapshotStatus.text");
    case "missing":
      return i18n.t("tasks:detail.labels.snapshotStatus.missing");
    case "binary":
      return i18n.t("tasks:detail.labels.snapshotStatus.binary");
    case "unavailable":
      return i18n.t("tasks:detail.labels.snapshotStatus.unavailable");
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
