import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { Loader2, Sparkles } from "lucide-react";

interface GitCommitDialogSummaryRow {
  label: string;
  value: ReactNode;
}

interface GitCommitDialogContentProps {
  title: string;
  description: string;
  summaryRows: GitCommitDialogSummaryRow[];
  commitMessage: string;
  busy: boolean;
  generatingCommitMessage: boolean;
  error?: string | null;
  placeholder?: string;
  footerStart?: ReactNode;
  extraActions?: ReactNode;
  generateDisabled?: boolean;
  submitDisabled?: boolean;
  submitLabel: string;
  onCommitMessageChange: (value: string) => void;
  onGenerateCommitMessage: () => void | Promise<void>;
  onCancel: () => void;
  onSubmit: () => void | Promise<void>;
}

export function GitCommitDialogContent({
  title,
  description,
  summaryRows,
  commitMessage,
  busy,
  generatingCommitMessage,
  error = null,
  placeholder = "输入提交信息…（Cmd/Ctrl+Enter 提交）",
  footerStart,
  extraActions,
  generateDisabled = false,
  submitDisabled = false,
  submitLabel,
  onCommitMessageChange,
  onGenerateCommitMessage,
  onCancel,
  onSubmit,
}: GitCommitDialogContentProps) {
  return (
    <DialogContent className="max-w-lg">
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{description}</DialogDescription>
      </DialogHeader>

      <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
        {summaryRows.map((row, index) => (
          <div key={row.label} className={index === 0 ? undefined : "mt-1"}>
            {row.label}：{row.value}
          </div>
        ))}
      </div>

      <div className="space-y-1.5">
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs font-medium text-muted-foreground">提交说明</span>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => void onGenerateCommitMessage()}
            disabled={busy || generateDisabled}
            title="AI 生成提交信息"
          >
            {generatingCommitMessage ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Sparkles className="h-4 w-4" />
            )}
          </Button>
        </div>
        <Textarea
          value={commitMessage}
          onChange={(event) => onCommitMessageChange(event.target.value)}
          disabled={busy}
          placeholder={placeholder}
          className="min-h-28"
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void onSubmit();
            }
          }}
        />
      </div>

      {error && (
        <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      <DialogFooter className={footerStart ? "gap-2 sm:justify-between" : undefined}>
        {footerStart}
        <div className="flex gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={onCancel}
            disabled={busy}
          >
            取消
          </Button>
          {extraActions}
          <Button
            type="button"
            onClick={() => void onSubmit()}
            disabled={busy || submitDisabled}
          >
            {submitLabel}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  );
}
