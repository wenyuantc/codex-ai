import i18n from "@/lib/i18n";

/**
 * Map stable Auto QC / automation `last_error` phrases from Rust into the UI locale.
 * Dynamic or unknown strings pass through unchanged.
 */
export function mapAutomationNote(message: string | null | undefined): string {
  const raw = (message ?? "").trim();
  if (!raw) {
    return "";
  }

  const t = (key: string, options?: Record<string, unknown>) =>
    i18n.t(`tasks:detail.overview.automationNotes.${key}`, options);

  const exact: Record<string, string> = {
    "审核结果结构化输出无效，自动质控已停止，需人工接管": "invalidReviewVerdict",
    审核结果结构化输出无效: "invalidReviewVerdictShort",
    "自动修复执行异常失败，需人工接管": "autoFixFailed",
    "执行已被人工停止，自动质控交由人工接管": "executionStoppedManual",
    "审核已被人工停止，自动质控交由人工接管": "reviewStoppedManual",
    执行已被人工停止: "executionStoppedShort",
    "审核已通过，任务未启用 Worktree，已跳过自动提交代码": "reviewPassedSkipCommitNoWorktree",
    "执行完成但没有产生可审核的代码改动，自动质控无法进入审核，需人工补充任务或重新执行":
      "noReviewableChanges",
    "应用启动时发现会话上次未正常收尾，已标记为失败": "orphanedSession",
    步骤已被人工停止: "pipelineStepStopped",
    编排已被人工停止: "pipelineStopped",
    步骤已被手动停止: "pipelineStepManualStopped",
    手动编排步骤已停止: "pipelineManualStepStopped",
    编排已转人工: "pipelineHandedToManual",
  };

  const key = exact[raw];
  if (key) {
    return t(key);
  }

  let match = raw.match(/^自动修复 (\d+) 轮后仍未通过审核：(.+)$/);
  if (match) {
    return t("maxFixRoundsFailedWithDetail", { rounds: match[1], detail: match[2] });
  }

  match = raw.match(/^自动修复 (\d+) 轮后仍未通过审核$/);
  if (match) {
    return t("maxFixRoundsFailed", { rounds: match[1] });
  }

  return raw;
}
