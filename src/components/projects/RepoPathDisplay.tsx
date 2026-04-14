import { useEffect, useMemo, useRef, useState } from "react";
import { Check, Copy, GitBranch, Link2Off } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface RepoPathDisplayProps {
  repoPath?: string | null;
  compact?: boolean;
  showCopyAction?: boolean;
  className?: string;
}

export function RepoPathDisplay({
  repoPath,
  compact = false,
  showCopyAction = false,
  className,
}: RepoPathDisplayProps) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);
  const fullPath = useMemo(() => repoPath?.trim() || "", [repoPath]);
  const canCopy =
    showCopyAction &&
    typeof navigator !== "undefined" &&
    typeof navigator.clipboard?.writeText === "function" &&
    !!fullPath;

  useEffect(() => {
    return () => {
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
    };
  }, []);

  const handleCopy = async () => {
    if (!fullPath || !canCopy) {
      return;
    }

    await navigator.clipboard.writeText(fullPath);
    setCopied(true);

    if (resetTimerRef.current !== null) {
      window.clearTimeout(resetTimerRef.current);
    }

    resetTimerRef.current = window.setTimeout(() => {
      setCopied(false);
    }, 1600);
  };

  if (compact) {
    return (
      <div className={cn("min-w-0", className)}>
        <Badge
          variant={fullPath ? "outline" : "secondary"}
          className={cn(
            "max-w-full rounded-full px-2 py-0 text-[11px]",
            fullPath ? "border-primary/20 bg-primary/5 text-primary" : ""
          )}
        >
          {fullPath ? (
            <>
              <GitBranch className="h-3 w-3" />
              Git 仓库
            </>
          ) : (
            <>
              <Link2Off className="h-3 w-3" />
              未配置
            </>
          )}
        </Badge>

        <code
          className="mt-1 block min-w-0 truncate text-[11px] text-muted-foreground"
          title={fullPath || "未配置仓库路径"}
        >
          {fullPath || "未配置仓库路径"}
        </code>
      </div>
    );
  }

  return (
    <div className={cn("flex min-w-0 items-center gap-2", className)}>
      <Badge
        variant={fullPath ? "outline" : "secondary"}
        className={cn(
          "shrink-0 rounded-full px-2 py-0 text-[11px]",
          fullPath ? "border-primary/20 bg-primary/5 text-primary" : ""
        )}
      >
        {fullPath ? (
          <>
            <GitBranch className="h-3 w-3" />
            Git 仓库
          </>
        ) : (
          <>
            <Link2Off className="h-3 w-3" />
            未配置
          </>
        )}
      </Badge>

      <code
        className={cn(
          "min-w-0 flex-1 truncate text-muted-foreground",
          "text-xs"
        )}
        title={fullPath || "未配置仓库路径"}
      >
        {fullPath || "未配置仓库路径"}
      </code>

      {canCopy ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="shrink-0 cursor-pointer text-muted-foreground hover:text-foreground"
          onClick={() => void handleCopy()}
          title={copied ? "已复制完整路径" : "复制完整路径"}
          aria-label={copied ? "已复制完整路径" : "复制完整路径"}
        >
          {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
        </Button>
      ) : null}
    </div>
  );
}
