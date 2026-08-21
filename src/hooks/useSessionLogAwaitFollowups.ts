import { useEffect, useRef } from "react";

import { can, getCachedAiProviderCapabilities } from "@/lib/aiCapabilities";
import { finishClaudeInput, setClaudeAwaitFollowups } from "@/lib/claude";
import { finishCodexInput, setCodexAwaitFollowups } from "@/lib/codex";
import { finishOpenCodeInput, setOpenCodeAwaitFollowups } from "@/lib/opencode";

async function setAwaitFollowups(
  provider: string,
  employeeId: string,
  enabled: boolean,
): Promise<void> {
  switch (provider) {
    case "claude":
      await setClaudeAwaitFollowups(employeeId, enabled);
      return;
    case "opencode":
      await setOpenCodeAwaitFollowups(employeeId, enabled);
      return;
    case "grok":
    case "native":
      return;
    case "codex":
      await setCodexAwaitFollowups(employeeId, enabled);
      return;
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}

async function endInteractiveSession(provider: string, employeeId: string): Promise<void> {
  switch (provider) {
    case "claude":
      await finishClaudeInput(employeeId);
      return;
    case "opencode":
      await finishOpenCodeInput(employeeId);
      return;
    case "grok":
    case "native":
      return;
    case "codex":
      await finishCodexInput(employeeId);
      return;
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}

/**
 * Terminal log open + live session → wait for user input after the turn.
 * Log closed while still live → end session so the task can auto-continue.
 */
export function useSessionLogAwaitFollowups(
  open: boolean,
  employeeId: string | null | undefined,
  provider: string | null | undefined,
  live: boolean,
): void {
  const armedRef = useRef(false);

  useEffect(() => {
    const providerKey = String(provider || "").trim();
    const id = employeeId?.trim() || "";
    if (!open || !id || !providerKey || !live) {
      return;
    }
    if (providerKey === "grok" || providerKey === "native") {
      return;
    }
    const caps = getCachedAiProviderCapabilities();
    if (caps && !can(caps, providerKey, "send_input")) {
      return;
    }

    let cancelled = false;
    armedRef.current = true;
    void setAwaitFollowups(providerKey, id, true).catch(() => {
      // Session may have exited before the control line was written.
    });

    return () => {
      cancelled = true;
      if (!armedRef.current) {
        return;
      }
      armedRef.current = false;
      // Closing the log while the process is still live: stop waiting and let the task continue.
      void endInteractiveSession(providerKey, id).catch(() => {
        if (cancelled) {
          return;
        }
      });
    };
  }, [open, employeeId, provider, live]);
}
