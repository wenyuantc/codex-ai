import { useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCheck, Loader2, Send } from "lucide-react";

import {
  can,
  capabilityDisabledReason,
  getCachedAiProviderCapabilities,
} from "@/lib/aiCapabilities";
import { finishClaudeInput, sendClaudeInput } from "@/lib/claude";
import { finishCodexInput, sendCodexInput } from "@/lib/codex";
import { sendGrokInput } from "@/lib/grok";
import { finishNativeInput, sendNativeInput } from "@/lib/native";
import { finishOpenCodeInput, sendOpenCodeInput } from "@/lib/opencode";
import type { AiProvider } from "@/lib/types";
import { useAiProviderCapabilities } from "@/hooks/useAiProviderCapabilities";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface SessionInputBarProps {
  employeeId: string | null | undefined;
  provider: AiProvider | string | null | undefined;
  /** When false, bar stays visible but disabled (e.g. filtered history view). */
  sessionLive?: boolean;
  className?: string;
}

async function dispatchSendInput(
  provider: string,
  employeeId: string,
  input: string,
): Promise<void> {
  switch (provider) {
    case "claude":
      await sendClaudeInput(employeeId, input);
      return;
    case "opencode":
      await sendOpenCodeInput(employeeId, input);
      return;
    case "grok":
      await sendGrokInput(employeeId, input);
      return;
    case "native":
      await sendNativeInput(employeeId, input);
      return;
    case "codex":
      await sendCodexInput(employeeId, input);
      return;
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}

export function SessionInputBar({
  employeeId,
  provider,
  sessionLive = true,
  className = "",
}: SessionInputBarProps) {
  const { t } = useTranslation("sessions");
  const { capabilities, loading } = useAiProviderCapabilities();
  const caps = capabilities ?? getCachedAiProviderCapabilities();
  const [value, setValue] = useState("");
  const [sending, setSending] = useState(false);
  const [finishing, setFinishing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const supported = can(caps, provider, "send_input");
  const disabledReason = capabilityDisabledReason(caps, provider, "send_input");
  const missingEmployee = !employeeId;
  const busy = sending || finishing;
  const canInteract = Boolean(employeeId) && supported && sessionLive && !loading && !busy;
  const enabled = canInteract && value.trim().length > 0;

  let title: string | undefined;
  if (missingEmployee) {
    title = t("inputMissingEmployee");
  } else if (!sessionLive) {
    title = t("inputNotLive");
  } else if (!loading && !supported) {
    title = disabledReason ?? t("inputUnsupported");
  } else if (sessionLive && supported && !loading) {
    title = t("inputLiveHint");
  }

  const dispatchFinishInput = async (providerKey: string, id: string): Promise<void> => {
    switch (providerKey) {
      case "claude":
        await finishClaudeInput(id);
        return;
      case "opencode":
        await finishOpenCodeInput(id);
        return;
      case "grok":
        throw new Error(t("inputFinishUnsupported"));
      case "native":
        await finishNativeInput(id);
        return;
      case "codex":
        await finishCodexInput(id);
        return;
      default:
        throw new Error(`未知 AI 引擎：${providerKey}`);
    }
  };

  const onSubmit = async () => {
    if (!employeeId || !enabled) {
      return;
    }
    const trimmed = value.trim();
    if (!trimmed) {
      return;
    }
    setSending(true);
    setError(null);
    try {
      await dispatchSendInput(String(provider || "codex"), employeeId, trimmed);
      setValue("");
    } catch (err) {
      setError(err instanceof Error ? err.message : t("inputSendFailed"));
    } finally {
      setSending(false);
    }
  };

  const onFinish = async () => {
    if (!employeeId || !canInteract) {
      return;
    }
    setFinishing(true);
    setError(null);
    try {
      await dispatchFinishInput(String(provider || "codex"), employeeId);
    } catch (err) {
      setError(err instanceof Error ? err.message : t("inputFinishFailed"));
    } finally {
      setFinishing(false);
    }
  };

  return (
    <div className={`rounded-b border border-t-0 border-zinc-800 bg-black/60 p-2 ${className}`}>
      <div className="flex items-center gap-2">
        <Input
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void onSubmit();
            }
          }}
          placeholder={t("inputPlaceholder")}
          disabled={!supported || !sessionLive || missingEmployee || loading || busy}
          title={title}
          className="h-8 border-zinc-700 bg-zinc-950 text-xs text-zinc-100 placeholder:text-zinc-500"
          aria-label={t("inputPlaceholder")}
        />
        <Button
          type="button"
          size="sm"
          className="h-8 shrink-0"
          disabled={!enabled}
          title={title}
          onClick={() => void onSubmit()}
        >
          {sending ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Send className="h-3.5 w-3.5" />
          )}
          <span className="ml-1">{t("inputSend")}</span>
        </Button>
        <Button
          type="button"
          size="sm"
          variant="destructive"
          className="h-8 shrink-0 bg-red-600 text-white hover:bg-red-700 focus-visible:border-red-600 focus-visible:ring-red-600/40 disabled:bg-red-600/50"
          disabled={!canInteract}
          title={t("inputFinishHint")}
          onClick={() => void onFinish()}
        >
          {finishing ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <CheckCheck className="h-3.5 w-3.5" />
          )}
          <span className="ml-1">{t("inputFinish")}</span>
        </Button>
      </div>
      {!loading && !supported && disabledReason ? (
        <p className="mt-1 text-[11px] text-zinc-300">{disabledReason}</p>
      ) : null}
      {!loading && supported && !sessionLive ? (
        <p className="mt-1 text-[11px] text-zinc-300">{t("inputNotLive")}</p>
      ) : null}
      {!loading && supported && sessionLive && !missingEmployee ? (
        <p className="mt-1 text-[11px] leading-snug text-zinc-200">{t("inputLiveHint")}</p>
      ) : null}
      {error ? <p className="mt-1 text-[11px] text-red-400">{error}</p> : null}
    </div>
  );
}
