import { Info } from "lucide-react";

import type { ArtifactCaptureMode } from "@/lib/types";
import { isArtifactCaptureLimited } from "@/lib/utils";

interface SshArtifactLimitedNoticeProps {
  /** When provided, notice only renders for limited capture modes. */
  artifactCaptureMode?: ArtifactCaptureMode | null;
  /** Force show even without a mode (e.g. generic empty SSH state). */
  force?: boolean;
  className?: string;
  compact?: boolean;
}

export function SshArtifactLimitedNotice({
  artifactCaptureMode,
  force = false,
  className = "",
  compact = false,
}: SshArtifactLimitedNoticeProps) {
  const shouldShow =
    force
    || (artifactCaptureMode != null && isArtifactCaptureLimited(artifactCaptureMode));

  if (!shouldShow) {
    return null;
  }

  if (compact) {
    return (
      <div
        className={`rounded border border-amber-500/30 bg-amber-500/10 px-2 py-1.5 text-[11px] text-amber-900 dark:text-amber-100 ${className}`}
        role="status"
      >
        <div className="flex items-start gap-1.5">
          <Info className="mt-0.5 h-3 w-3 shrink-0" />
          <div className="space-y-1">
            <p className="font-medium">远程变更明细受限</p>
            <p className="text-amber-800/90 dark:text-amber-200/90">
              SSH 远程执行时，本地数据库通常只保留对话元数据；完整文件快照可能不可用。仍可查看日志、继续对话，或在远程主机检查 git 状态。升级远程 Codex SDK（ssh_full）或到远程机器审阅可获得更完整明细。
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-xs text-amber-900 dark:text-amber-100 ${className}`}
      role="status"
    >
      <div className="flex items-start gap-2">
        <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <div className="space-y-2">
          <p className="font-medium">远程变更明细受限（SSH）</p>
          <ul className="list-disc space-y-1.5 pl-4 text-amber-800/95 dark:text-amber-200/95">
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">为何受限：</span>
              当前为 SSH 远程执行。本地数据库主要保存会话元数据；完整文件 diff / 快照不一定会回传到本机。
            </li>
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">你仍可以：</span>
              查看会话日志、继续对话，并在远程主机上检查 git status / diff。
            </li>
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">如何获得更完整明细：</span>
              升级远程 Codex SDK 以启用 ssh_full 采集（可用时），或直接在远程机器上审阅变更。
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
