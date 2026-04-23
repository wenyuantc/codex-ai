import type { ProjectGitWorkingTreeChange } from "@/lib/types";

export type GitActionButtonTone =
  | "neutral"
  | "positive"
  | "create"
  | "info"
  | "warning"
  | "rollback"
  | "merge"
  | "danger";

export function getGitActionButtonClassName(tone: GitActionButtonTone) {
  switch (tone) {
    case "positive":
      return "border-emerald-500/45 bg-emerald-500/10 text-emerald-800 hover:bg-emerald-500/15 hover:text-emerald-900";
    case "create":
      return "border-teal-500/45 bg-teal-500/10 text-teal-800 hover:bg-teal-500/15 hover:text-teal-900";
    case "info":
      return "border-sky-500/45 bg-sky-500/10 text-sky-800 hover:bg-sky-500/15 hover:text-sky-900";
    case "warning":
      return "border-amber-500/45 bg-amber-500/10 text-amber-900 hover:bg-amber-500/15 hover:text-amber-950";
    case "rollback":
      return "border-orange-500/45 bg-orange-500/10 text-orange-800 hover:bg-orange-500/15 hover:text-orange-900";
    case "merge":
      return "border-violet-500/45 bg-violet-500/10 text-violet-800 hover:bg-violet-500/15 hover:text-violet-900";
    case "danger":
      return "border-rose-500/45 bg-rose-500/10 text-rose-800 hover:bg-rose-500/15 hover:text-rose-900";
    case "neutral":
    default:
      return "border-slate-400/35 bg-slate-500/5 text-slate-700 hover:bg-slate-500/10 hover:text-slate-900";
  }
}

export function getWorkingTreeChangeLabel(changeType: ProjectGitWorkingTreeChange["change_type"]) {
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

export function getWorkingTreeChangeClassName(changeType: ProjectGitWorkingTreeChange["change_type"]) {
  switch (changeType) {
    case "added":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
    case "modified":
      return "border-sky-500/30 bg-sky-500/10 text-sky-700";
    case "deleted":
      return "border-rose-500/30 bg-rose-500/10 text-rose-700";
    case "renamed":
      return "border-amber-500/30 bg-amber-500/10 text-amber-700";
    default:
      return "border-border/60 bg-secondary/40 text-foreground";
  }
}

export function getWorkingTreeStageStatusLabel(stageStatus: ProjectGitWorkingTreeChange["stage_status"]) {
  switch (stageStatus) {
    case "staged":
      return "已暂存";
    case "unstaged":
      return "未暂存";
    case "partially_staged":
      return "部分暂存";
    case "untracked":
      return "未跟踪";
    default:
      return stageStatus;
  }
}

export function getWorkingTreeStageStatusClassName(stageStatus: ProjectGitWorkingTreeChange["stage_status"]) {
  switch (stageStatus) {
    case "staged":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
    case "partially_staged":
      return "border-amber-500/30 bg-amber-500/10 text-amber-700";
    case "untracked":
      return "border-violet-500/30 bg-violet-500/10 text-violet-700";
    case "unstaged":
    default:
      return "border-border/60 bg-secondary/40 text-foreground";
  }
}
