import i18n from "@/lib/i18n";

/**
 * Map stable backend runtime/status Chinese phrases to the active UI locale.
 * Unknown / dynamic Err strings pass through unchanged.
 */
export function mapRuntimeStatusMessage(message: string | null | undefined): string {
  const raw = (message ?? "").trim();
  if (!raw) {
    return "";
  }

  const t = (key: string, options?: Record<string, unknown>) =>
    i18n.t(`settings:runtime.statusMessages.${key}`, options);

  if (raw === "应用启动时发现会话上次未正常收尾，已标记为失败") {
    return t("orphanedSession");
  }

  let match = raw.match(
    /^Codex SDK 已就绪，任务运行与一次性 AI 将优先使用 SDK[（(]Node (.+?)[）)]$/,
  );
  if (match) {
    return t("codexSdkReady", { version: match[1] });
  }

  match = raw.match(
    /^远程 Codex SDK 已就绪，任务运行与一次性 AI 将优先使用远程 SDK，失败时自动回退到远程 codex exec。$/,
  );
  if (match) {
    return t("remoteCodexSdkReady");
  }

  match = raw.match(/^Claude SDK 已就绪[（(]Node (.+?)[）)]$/);
  if (match) {
    return t("claudeSdkReady", { version: match[1] });
  }

  match = raw.match(/^OpenCode SDK 已就绪[（(]Node (.+?)[）)]$/);
  if (match) {
    return t("opencodeSdkReady", { version: match[1] });
  }

  match = raw.match(/^远程 OpenCode SDK 已就绪[（(]Node (.+?)[）)]$/);
  if (match) {
    return t("remoteOpencodeSdkReady", { version: match[1] });
  }

  match = raw.match(/^Grok CLI 可用[（(](.+?)[）)]，已登录$/);
  if (match) {
    return t("grokCliAvailableLoggedIn", { version: match[1] });
  }

  match = raw.match(/^远程 Grok CLI 可用[（(](.+?)[）)]，已登录$/);
  if (match) {
    return t("remoteGrokCliAvailableLoggedIn", { version: match[1] });
  }

  match = raw.match(/^远程 Grok CLI 可用，已登录$/);
  if (match) {
    return t("remoteGrokCliAvailableLoggedInNoVersion");
  }

  match = raw.match(
    /^Grok CLI 可用[（(](.+?)[）)]，但未登录或登录失效。请执行 `grok login`。(?: 详情：(.+))?$/,
  );
  if (match) {
    return match[2]
      ? t("grokCliAvailableNotLoggedInWithDetail", { version: match[1], detail: match[2] })
      : t("grokCliAvailableNotLoggedIn", { version: match[1] });
  }

  match = raw.match(
    /^远程 Grok CLI 可用[（(](.+?)[）)]，但未登录或登录失效。请在远程执行 `grok login`。$/,
  );
  if (match) {
    return t("remoteGrokCliAvailableNotLoggedIn", { version: match[1] });
  }

  match = raw.match(/^Grok CLI 可用[（(](.+?)[）)]；登录态未知$/);
  if (match) {
    return t("grokCliAvailableAuthUnknown", { version: match[1] });
  }

  match = raw.match(/^Grok CLI 可用[（(](.+?)[）)]；登录态探测失败：(.+)$/);
  if (match) {
    return t("grokCliAvailableAuthProbeFailed", { version: match[1], detail: match[2] });
  }

  match = raw.match(/^Codex SDK 未启用，任务运行与一次性 AI 将使用 codex exec$/);
  if (match) {
    return t("codexSdkDisabled");
  }

  match = raw.match(/^Codex SDK 未安装，已回退到 codex exec$/);
  if (match) {
    return t("codexSdkNotInstalled");
  }

  match = raw.match(/^Claude SDK 未安装，已回退到 Claude CLI$/);
  if (match) {
    return t("claudeSdkNotInstalledFallbackCli");
  }

  match = raw.match(/^OpenCode SDK 未启用$/);
  if (match) {
    return t("opencodeSdkDisabled");
  }

  match = raw.match(/^OpenCode SDK 未安装$/);
  if (match) {
    return t("opencodeSdkNotInstalled");
  }

  match = raw.match(/^一次性 AI 未启用 Claude SDK，将使用 Claude CLI$/);
  if (match) {
    return t("oneShotClaudeSdkDisabledUseCli");
  }

  match = raw.match(/^一次性 AI 未启用 OpenCode SDK，当前不可用$/);
  if (match) {
    return t("oneShotOpencodeSdkDisabled");
  }

  match = raw.match(/^内置 Agent 一次性 AI 将通过 AI 渠道「(.+?)」执行$/);
  if (match) {
    return t("oneShotNativeChannelReady", { channel: match[1] });
  }

  match = raw.match(/^一次性 AI 使用内置 Agent 时请先选择 AI 渠道$/);
  if (match) {
    return t("oneShotNativeChannelMissing");
  }

  match = raw.match(/^一次性 AI 绑定的 AI 渠道已停用$/);
  if (match) {
    return t("oneShotNativeChannelDisabled");
  }

  match = raw.match(/^SSH 模式下内置 Agent 一次性 AI 通过本地 AI 渠道执行$/);
  if (match) {
    return t("oneShotNativeChannelRemote");
  }

  if (raw === "已取消") {
    return t("cancelled");
  }
  if (raw === "达到最大模型轮次") {
    return t("maxModelTurns");
  }
  if (raw.startsWith("重复调用被拒绝")) {
    return t("repeatedToolCallRejected");
  }
  if (raw === "用户不允许该高风险操作") {
    return t("highRiskDenied");
  }
  if (raw === "确认超时，已按拒绝处理") {
    return t("permissionTimedOut");
  }

  return raw;
}
