import type {
  CodexSessionFileChange,
  ReviewFinding,
  TaskExecutionChangeHistoryItem,
} from "@/lib/types";

export function normalizeReviewPath(path: string): string {
  return path.trim().replace(/\\/g, "/").replace(/^\.\//, "");
}

function isPathSuffixMatch(left: string, right: string): boolean {
  if (!left || !right) {
    return false;
  }
  return left === right || left.endsWith(`/${right}`) || right.endsWith(`/${left}`);
}

function changeMatchesFinding(change: CodexSessionFileChange, file: string): boolean {
  const path = normalizeReviewPath(change.path);
  if (path === file) {
    return true;
  }
  const previousPath = change.previous_path ? normalizeReviewPath(change.previous_path) : "";
  if (previousPath && previousPath === file) {
    return true;
  }
  return (
    isPathSuffixMatch(path, file) || Boolean(previousPath && isPathSuffixMatch(previousPath, file))
  );
}

export function matchReviewFindingToChange(
  finding: ReviewFinding,
  history: TaskExecutionChangeHistoryItem[],
): CodexSessionFileChange | null {
  const file = normalizeReviewPath(finding.file);
  if (!file) {
    return null;
  }

  const ordered = [...history].sort((left, right) =>
    right.session.started_at.localeCompare(left.session.started_at),
  );

  for (const item of ordered) {
    const exactPath = item.changes.find((change) => normalizeReviewPath(change.path) === file);
    if (exactPath) {
      return exactPath;
    }
    const exactPrevious = item.changes.find(
      (change) => change.previous_path && normalizeReviewPath(change.previous_path) === file,
    );
    if (exactPrevious) {
      return exactPrevious;
    }
    const suffixMatch = item.changes.find((change) => changeMatchesFinding(change, file));
    if (suffixMatch) {
      return suffixMatch;
    }
  }

  return null;
}
