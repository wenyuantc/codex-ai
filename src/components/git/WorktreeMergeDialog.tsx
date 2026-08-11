import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

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
  const { t } = useTranslation("projects");
  const [targetBranch, setTargetBranch] = useState("");
  const [autoStash, setAutoStash] = useState(true);
  const [deleteWorktree, setDeleteWorktree] = useState(false);
  const [deleteBranch, setDeleteBranch] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const sourceBranch = worktree?.branch?.trim() ?? "";
  const hasWorkingTreeChanges = (worktree?.working_tree_changes.length ?? 0) > 0;
  const targetBranches = useMemo(
    () =>
      dedupeBranches([defaultBranch, currentBranch, ...projectBranches, sourceBranch]).filter(
        (branch) => branch !== sourceBranch,
      ),
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
      setError(t("worktreeMergeDialog.mergeNoWorktreeError"));
      return;
    }
    if (!sourceBranch) {
      setError(t("worktreeMergeDialog.mergeNoBranchError"));
      return;
    }
    if (!targetBranch) {
      setError(t("worktreeMergeDialog.mergeNoTargetError"));
      return;
    }
    if (targetBranch === sourceBranch) {
      setError(t("worktreeMergeDialog.mergeSameBranchError"));
      return;
    }
    if (deleteWorktree && hasWorkingTreeChanges && !autoStash) {
      setError(t("worktreeMergeDialog.mergeDeleteStashError"));
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
    submitting ||
    !worktree ||
    !sourceBranch ||
    !targetBranch ||
    targetBranch === sourceBranch ||
    (deleteWorktree && hasWorkingTreeChanges && !autoStash);

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
          <DialogTitle>{t("worktreeMergeDialog.title")}</DialogTitle>
          <DialogDescription>{t("worktreeMergeDialog.description")}</DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>
            {t("worktreeMergeDialog.sourceBranchLabel", {
              branch: sourceBranch || t("worktreeMergeDialog.unboundBranch"),
            })}
          </div>
          <div className="mt-1">
            {t("worktreeMergeDialog.worktreeLabel")}：
            <span className="break-all font-mono">
              {worktree?.path ?? t("worktree.unknownBranch")}
            </span>
          </div>
          <div className="mt-1">
            {t("worktreeMergeDialog.uncommittedChanges")}
            {hasWorkingTreeChanges
              ? t("worktreeMergeDialog.changeCount", {
                  count: worktree?.working_tree_changes.length ?? 0,
                })
              : t("worktreeMergeDialog.none")}
          </div>
        </div>

        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-muted-foreground">
            {t("worktreeMergeDialog.mergeToBranch")}
          </span>
          <Select<string>
            value={targetBranch}
            onValueChange={(value) => value && setTargetBranch(value)}
            disabled={submitting || targetBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{targetBranch || t("gitActionDialog.selectTargetBranch")}</SelectValue>
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
            <span>{t("worktreeMergeDialog.autoStash")}</span>
          </label>

          <label
            className={`flex items-start gap-3 rounded-md border border-border/60 px-3 py-2 text-sm ${
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
            <span>{t("worktreeMergeDialog.deleteWorktreeAfter")}</span>
          </label>

          <label
            className={`flex items-start gap-3 rounded-md border border-border/60 px-3 py-2 text-sm ${
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
            <span>
              {t("worktreeMergeDialog.deleteBranchAfter", {
                branch: sourceBranch || t("worktreeMergeDialog.deleteSourceBranch"),
              })}
            </span>
          </label>
        </div>

        {hasWorkingTreeChanges && autoStash && (
          <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
            {t("worktreeMergeDialog.autoStashHint")}
          </div>
        )}

        {worktree?.is_locked && (
          <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
            {t("worktreeMergeDialog.lockedHint")}
          </div>
        )}

        {deleteWorktree && hasWorkingTreeChanges && !autoStash && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {t("worktreeMergeDialog.deleteBlockedHint")}
          </div>
        )}

        {targetBranches.length === 0 && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {t("worktreeMergeDialog.noTargetBranches")}
          </div>
        )}

        {error && (
          <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            {t("common:cancel")}
          </Button>
          <Button type="button" onClick={() => void handleSubmit()} disabled={submitDisabled}>
            {submitting ? t("worktreeMergeDialog.merging") : t("worktreeMergeDialog.mergeNow")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
