import { Info } from "lucide-react";
import { useTranslation } from "react-i18next";

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
  const { t } = useTranslation("ssh");
  const shouldShow =
    force || (artifactCaptureMode != null && isArtifactCaptureLimited(artifactCaptureMode));

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
            <p className="font-medium">{t("artifactLimitedTitle")}</p>
            <p className="text-amber-800/90 dark:text-amber-200/90">{t("artifactLimitedBody")}</p>
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
          <p className="font-medium">{t("artifactLimitedTitleSsh")}</p>
          <ul className="list-disc space-y-1.5 pl-4 text-amber-800/95 dark:text-amber-200/95">
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">
                {t("artifactLimitedWhyLabel")}
              </span>
              {t("artifactLimitedWhy")}
            </li>
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">
                {t("artifactLimitedCanLabel")}
              </span>
              {t("artifactLimitedCan")}
            </li>
            <li>
              <span className="font-medium text-amber-950 dark:text-amber-50">
                {t("artifactLimitedHowLabel")}
              </span>
              {t("artifactLimitedHow")}
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
