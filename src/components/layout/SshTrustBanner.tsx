import { Link } from "react-router-dom";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

interface SshTrustBannerProps {
  className?: string;
}

/**
 * Global sticky banner shown when environmentMode === "ssh".
 * Communicates that review evidence may be incomplete; links to settings.
 * Session-level limited capture still uses SshArtifactLimitedNotice.
 */
export function SshTrustBanner({ className }: SshTrustBannerProps) {
  const { t } = useTranslation("ssh");

  return (
    <div
      className={cn(
        "border-b border-amber-500/30 bg-amber-500/10 px-6 py-2 text-xs text-amber-950 dark:text-amber-100",
        className,
      )}
      role="status"
      data-slot="ssh-trust-banner"
    >
      <div className="flex flex-wrap items-start gap-2">
        <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-700 dark:text-amber-200" />
        <div className="min-w-0 flex-1 space-y-0.5">
          <p>
            <span className="font-medium">{t("trustBannerTitle")}</span>
            {t("trustBanner")}
          </p>
          <p className="text-amber-900/85 dark:text-amber-100/85">
            {t("trustBannerHint")}
            <Link
              to="/settings"
              className="ml-1 font-medium text-amber-950 underline underline-offset-2 hover:text-amber-800 dark:text-amber-50 dark:hover:text-amber-100"
            >
              {t("openSettings")}
            </Link>
          </p>
        </div>
      </div>
    </div>
  );
}
