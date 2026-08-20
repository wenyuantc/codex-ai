import i18n from "@/lib/i18n";
import type { AiProvider, ProjectType } from "@/lib/types";

export type ImageSkipReason =
  "ssh_grok" | "ssh_claude" | "claude_cli" | "ssh_no_task" | "batch_mixed";

export class ImageSkipCancelledError extends Error {
  constructor() {
    super("image-skip-cancelled");
    this.name = "ImageSkipCancelledError";
  }
}

export function isImageSkipCancelled(error: unknown): boolean {
  return error instanceof ImageSkipCancelledError;
}

export function resolveImageAttachmentSkip(input: {
  imageCount: number;
  provider: AiProvider | string | null | undefined;
  projectType: ProjectType | string | null | undefined;
  claudeEffectiveProvider?: string | null;
  hasTaskId?: boolean;
}): ImageSkipReason | null {
  if (input.imageCount <= 0) {
    return null;
  }

  const provider = input.provider || "codex";
  const isSsh = input.projectType === "ssh";

  if (provider === "grok" && isSsh) {
    return "ssh_grok";
  }
  if (provider === "claude" && isSsh) {
    return "ssh_claude";
  }
  if (provider === "claude" && input.claudeEffectiveProvider !== "sdk") {
    return "claude_cli";
  }
  if ((provider === "codex" || provider === "opencode") && isSsh && input.hasTaskId === false) {
    return "ssh_no_task";
  }
  return null;
}

export function imageSkipMessage(reason: ImageSkipReason, count: number): string {
  return i18n.t(`tasks:imageSkip.${reason}`, { count });
}

type PendingSkipConfirm = {
  reason: ImageSkipReason;
  count: number;
  resolve: (ok: boolean) => void;
};

let pending: PendingSkipConfirm | null = null;
let notify: (() => void) | null = null;

export function subscribeImageSkipConfirm(listener: () => void): () => void {
  notify = listener;
  return () => {
    if (notify === listener) {
      notify = null;
    }
  };
}

export function getPendingImageSkipConfirm(): PendingSkipConfirm | null {
  return pending;
}

export function completeImageSkipConfirm(ok: boolean): void {
  const current = pending;
  pending = null;
  current?.resolve(ok);
  notify?.();
}

export function confirmImageAttachmentSkip(
  reason: ImageSkipReason,
  count: number,
): Promise<boolean> {
  if (pending) {
    pending.resolve(false);
    pending = null;
  }
  return new Promise((resolve) => {
    pending = { reason, count, resolve };
    if (!notify) {
      resolve(window.confirm(imageSkipMessage(reason, count)));
      pending = null;
      return;
    }
    notify();
  });
}
