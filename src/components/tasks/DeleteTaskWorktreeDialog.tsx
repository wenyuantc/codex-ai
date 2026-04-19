import { useEffect, useState } from "react";

import { confirmGitAction, requestGitAction } from "@/lib/backend";
import type { TaskGitContext } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface DeleteTaskWorktreeDialogProps {
  open: boolean;
  context: TaskGitContext | null;
  onOpenChange: (open: boolean) => void;
  onCompleted?: (message: string) => Promise<void> | void;
}

export function DeleteTaskWorktreeDialog({
  open,
  context,
  onOpenChange,
  onCompleted,
}: DeleteTaskWorktreeDialogProps) {
  const [deleteBranch, setDeleteBranch] = useState(true);
  const [forceRemove, setForceRemove] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const branchName = context?.task_branch?.trim() ?? "";
  const canDeleteBranch = branchName.length > 0;

  useEffect(() => {
    if (!open) {
      return;
    }

    setDeleteBranch(canDeleteBranch);
    setForceRemove(false);
    setSubmitting(false);
    setError(null);
  }, [canDeleteBranch, open]);

  const handleConfirm = async () => {
    if (!context) {
      return;
    }

    setSubmitting(true);
    setError(null);

    try {
      const request = await requestGitAction(context.id, "cleanup_worktree", {
        delete_branch: canDeleteBranch ? deleteBranch : false,
        prune_worktree: true,
        force_remove: forceRemove,
      });
      const result = await confirmGitAction(context.id, request.token);
      await onCompleted?.(result.message);
      onOpenChange(false);
    } catch (confirmError) {
      setError(confirmError instanceof Error ? confirmError.message : String(confirmError));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => {
      if (!submitting) {
        onOpenChange(nextOpen);
      }
    }}
    >
      <DialogContent className="w-[min(96vw,36rem)] max-w-[min(96vw,36rem)] p-0" showCloseButton={!submitting}>
        <div className="px-6 pt-6">
          <DialogHeader>
            <DialogTitle className="text-[22px] leading-none">删除 Worktree</DialogTitle>
          </DialogHeader>

          <div className="mt-3 space-y-1.5 text-base text-foreground">
            <p>
              确定要删除 worktree <span className="font-semibold text-foreground">{branchName || "未命名分支"}</span> 吗？
            </p>
            <p className="text-xl leading-none text-destructive">这将删除目录及其中所有文件，此操作不可撤销！</p>
          </div>

          <div className="mt-6 space-y-5">
            <label className={`flex items-start gap-3 text-[15px] ${canDeleteBranch ? "text-foreground" : "text-muted-foreground"}`}>
              <input
                type="checkbox"
                checked={canDeleteBranch ? deleteBranch : false}
                onChange={(event) => setDeleteBranch(event.target.checked)}
                disabled={!canDeleteBranch || submitting}
                className="mt-1 h-4 w-4 rounded border-border accent-primary"
              />
              <span>
                同时删除分支{" "}
                <span className="font-semibold">{branchName || "当前未绑定分支"}</span>
              </span>
            </label>

            <label className="flex items-start gap-3 text-[15px] text-muted-foreground">
              <input
                type="checkbox"
                checked={forceRemove}
                onChange={(event) => setForceRemove(event.target.checked)}
                disabled={submitting}
                className="mt-1 h-4 w-4 rounded border-border accent-primary"
              />
              <span>强制删除（忽略未提交的修改）</span>
            </label>
          </div>

          {error ? (
            <div className="mt-5 rounded-lg border border-destructive/20 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          ) : null}
        </div>

        <DialogFooter className="mt-6 px-6 py-5">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={() => void handleConfirm()}
            disabled={submitting || !context}
          >
            {submitting ? "删除中..." : "删除"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
