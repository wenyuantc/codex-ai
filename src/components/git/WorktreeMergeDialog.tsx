import { useEffect, useMemo, useState } from "react";

import { mergeProjectGitWorktree } from "@/lib/backend";
import type { ProjectGitWorktree } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface WorktreeMergeDialogProps {
  open: boolean;
  projectId: string;
  worktree: ProjectGitWorktree | null;
  currentBranch?: string | null;
  defaultBranch?: string | null;
  projectBranches: string[];
  onOpenChange: (open: boolean) => void;
  onMerged?: (message: string) => Promise<void> | void;
}

function dedupeBranches(values: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of values) {
    const value = raw?.trim();
    if (!value || seen.has(value)) {
      continue;
    }
    seen.add(value);
    out.push(value);
  }
  return out;
}

export function WorktreeMergeDialog({
  open,
  projectId,
  worktree,
  currentBranch,
  defaultBranch,
  projectBranches,
  onOpenChange,
  onMerged,
}: WorktreeMergeDialogProps) {
  const [targetBranch, setTargetBranch] = useState("");
  const [autoStash, setAutoStash] = useState(true);
  const [deleteWorktree, setDeleteWorktree] = useState(false);
  const [deleteBranch, setDeleteBranch] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const sourceBranch = worktree?.branch?.trim() ?? "";
  const hasWorkingTreeChanges = (worktree?.working_tree_changes.length ?? 0) > 0;
  const targetBranches = useMemo(
    () => dedupeBranches([defaultBranch, currentBranch, ...projectBranches, sourceBranch])
      .filter((branch) => branch !== sourceBranch),
    [currentBranch, defaultBranch, projectBranches, sourceBranch],
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    setTargetBranch(targetBranches[0] ?? "");
    setAutoStash(true);
    setDeleteWorktree(false);
    setDeleteBranch(false);
    setSubmitting(false);
    setError(null);
  }, [open, targetBranches, worktree?.path]);

  const handleSubmit = async () => {
    if (!worktree) {
      setError("当前 worktree 不存在，无法执行合并。");
      return;
    }
    if (!sourceBranch) {
      setError("当前 worktree 未绑定分支，无法执行合并。");
      return;
    }
    if (!targetBranch) {
      setError("请选择目标分支。");
      return;
    }
    if (targetBranch === sourceBranch) {
      setError("源分支和目标分支不能相同。");
      return;
    }
    if (deleteWorktree && hasWorkingTreeChanges && !autoStash) {
      setError("若要在合并后删除 worktree，请先勾选“自动暂存未提交的更改”。");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const message = await mergeProjectGitWorktree(
        projectId,
        worktree.path,
        targetBranch,
        autoStash,
        deleteWorktree,
        deleteBranch,
      );
      await onMerged?.(message);
      onOpenChange(false);
    } catch (mergeError) {
      setError(mergeError instanceof Error ? mergeError.message : String(mergeError));
    } finally {
      setSubmitting(false);
    }
  };

  const submitDisabled =
    submitting
    || !worktree
    || !sourceBranch
    || !targetBranch
    || targetBranch === sourceBranch
    || (deleteWorktree && hasWorkingTreeChanges && !autoStash);

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!submitting) {
          onOpenChange(nextOpen);
        }
      }}
    >
      <DialogContent className="max-w-lg" showCloseButton={!submitting}>
        <DialogHeader>
          <DialogTitle>合并 Worktree</DialogTitle>
          <DialogDescription>
            将当前 worktree 对应分支合并到指定目标分支；合并完成后可顺手删除 worktree 与来源分支。
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>来源分支：{sourceBranch || "未绑定分支"}</div>
          <div className="mt-1">
            Worktree：<span className="break-all font-mono">{worktree?.path ?? "未知"}</span>
          </div>
          <div className="mt-1">
            未提交变更：{hasWorkingTreeChanges ? `${worktree?.working_tree_changes.length ?? 0} 项` : "无"}
          </div>
        </div>

        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-muted-foreground">合并到分支</span>
          <Select<string>
            value={targetBranch}
            onValueChange={(value) => value && setTargetBranch(value)}
            disabled={submitting || targetBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{targetBranch || "选择目标分支"}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {targetBranches.map((branch) => (
                <SelectItem key={branch} value={branch}>
                  {branch}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>

        <div className="space-y-3">
          <label className="flex items-start gap-3 rounded-md border border-border/60 px-3 py-2 text-sm">
            <input
              type="checkbox"
              checked={autoStash}
              onChange={(event) => setAutoStash(event.target.checked)}
              disabled={submitting}
              className="mt-1 h-4 w-4 rounded border-border accent-primary"
            />
            <span>自动暂存未提交的更改</span>
          </label>

          <label className={`flex items-start gap-3 rounded-md border border-border/60 px-3 py-2 text-sm ${
            worktree?.is_locked ? "text-muted-foreground" : ""
          }`}
          >
            <input
              type="checkbox"
              checked={deleteWorktree}
              onChange={(event) => {
                const checked = event.target.checked;
                setDeleteWorktree(checked);
                if (!checked) {
                  setDeleteBranch(false);
                }
              }}
              disabled={submitting || worktree?.is_locked}
              className="mt-1 h-4 w-4 rounded border-border accent-primary"
            />
            <span>合并后删除 worktree</span>
          </label>

          <label className={`flex items-start gap-3 rounded-md border border-border/60 px-3 py-2 text-sm ${
            !deleteWorktree ? "text-muted-foreground" : ""
          }`}
          >
            <input
              type="checkbox"
              checked={deleteBranch}
              onChange={(event) => {
                const checked = event.target.checked;
                setDeleteBranch(checked);
                if (checked) {
                  setDeleteWorktree(true);
                }
              }}
              disabled={submitting || !deleteWorktree || !sourceBranch}
              className="mt-1 h-4 w-4 rounded border-border accent-primary"
            />
            <span>合并后删除分支 {sourceBranch || "当前来源分支"}</span>
          </label>
        </div>

        {hasWorkingTreeChanges && autoStash && (
          <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
            检测到当前 worktree 有未提交改动，合并前会先暂存这些改动（含未跟踪文件），合并完成后不会自动恢复。
          </div>
        )}

        {worktree?.is_locked && (
          <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
            当前 worktree 已锁定，本次可以执行合并，但不能在合并后直接删除。
          </div>
        )}

        {deleteWorktree && hasWorkingTreeChanges && !autoStash && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            当前 worktree 仍有未提交改动；如果要在合并后删除 worktree，请勾选“自动暂存未提交的更改”。
          </div>
        )}

        {targetBranches.length === 0 && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            当前没有可供选择的目标分支。
          </div>
        )}

        {error && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={submitting}>
            取消
          </Button>
          <Button type="button" onClick={() => void handleSubmit()} disabled={submitDisabled}>
            {submitting ? "合并中..." : "立即合并"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
