import type { ProjectGitWorkingTreeChange } from "@/lib/types";

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
