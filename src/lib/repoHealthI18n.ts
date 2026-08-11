import i18n from "@/lib/i18n";
import type { ProjectRepoHealth, ProjectRepoHealthCheck } from "@/lib/backend";

const NS = "projects:detailPage.repoHealth";

function t(key: string, options?: Record<string, unknown>): string {
  return i18n.t(`${NS}.${key}`, options);
}

export function getRepoHealthMessage(health: ProjectRepoHealth): string {
  if (health.project_type === "ssh") {
    return health.accessible ? t("sshConfigOk") : t("sshConfigIncomplete");
  }

  if (!health.path_exists) {
    return t("pathMissing");
  }
  if (!health.is_git_repo) {
    return t("notGitRepo");
  }

  const branch = health.current_branch?.trim() || t("branchUnknown");
  const worktree = health.is_dirty === true ? t("worktreeDirtyShort") : t("worktreeCleanShort");
  return t("repoOk", { branch, worktree });
}

export function getRepoHealthCheckLabel(check: ProjectRepoHealthCheck): string {
  return t(`checks.${check.key}`, { defaultValue: check.label });
}

export function getRepoHealthCheckDetail(
  health: ProjectRepoHealth,
  check: ProjectRepoHealthCheck,
): string {
  switch (check.key) {
    case "ssh_config":
      return check.passed ? t("detail.sshBound") : t("detail.sshUnbound");
    case "remote_path":
      return check.passed ? (health.path ?? check.detail) : t("detail.remotePathMissing");
    case "remote_git":
      return t("detail.remoteGitHint");
    case "path_exists":
      return check.passed
        ? (health.path ?? check.detail)
        : t("detail.pathMissing", { path: health.path ?? "" });
    case "is_directory":
      return check.passed ? t("detail.dirOk") : t("detail.dirBad");
    case "is_git_repo":
      return check.passed ? t("detail.gitOk") : t("detail.gitMissing");
    case "current_branch":
      return check.detail;
    case "worktree_dirty": {
      if (!check.passed) return check.detail;
      if (health.is_dirty === true) {
        const match = check.detail.match(/(\d+)/);
        const count = match ? Number(match[1]) : undefined;
        if (count != null && !Number.isNaN(count)) {
          return t("detail.worktreeDirtyLines", { count });
        }
        return t("worktreeDirtyShort");
      }
      return t("detail.worktreeClean");
    }
    default:
      return check.detail;
  }
}
