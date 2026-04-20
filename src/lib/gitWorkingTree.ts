import type { ProjectGitWorkingTreeChange } from "./types";

function isStageableChange(change: ProjectGitWorkingTreeChange) {
  return (
    change.stage_status === "unstaged"
    || change.stage_status === "untracked"
    || change.stage_status === "partially_staged"
  );
}

function isStagedChange(change: ProjectGitWorkingTreeChange) {
  return (
    change.stage_status === "staged"
    || change.stage_status === "partially_staged"
  );
}

export function countStageableGitFiles(changes: ProjectGitWorkingTreeChange[]) {
  return changes.filter(isStageableChange).length;
}

export function countStagedGitFiles(changes: ProjectGitWorkingTreeChange[]) {
  return changes.filter(isStagedChange).length;
}

export function buildGitCommitChangePrompts(changes: ProjectGitWorkingTreeChange[]) {
  return changes
    .filter(isStagedChange)
    .map((change) => {
      if (change.change_type === "renamed" && change.previous_path) {
        return `重命名 ${change.previous_path} -> ${change.path}`;
      }

      const label =
        change.change_type === "added"
          ? "新增"
          : change.change_type === "deleted"
            ? "删除"
            : change.change_type === "renamed"
              ? "重命名"
              : "修改";

      return `${label} ${change.path}`;
    });
}
