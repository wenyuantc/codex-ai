import { useEffect, useMemo, useState } from "react";

import {
  commitTaskGitChanges,
  getTaskGitCommitOverview,
  stageAllTaskGitFiles,
} from "@/lib/backend";
import { aiGenerateCommitMessage } from "@/lib/codex";
import type {
  ProjectGitWorkingTreeChange,
  Task,
  TaskGitCommitOverview,
  TaskGitContext,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { Loader2, Sparkles } from "lucide-react";

interface TaskGitCommitDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  task: Task;
  gitContext: TaskGitContext;
  onCommitted?: (message: string) => Promise<void> | void;
}

function buildStagedChangePrompts(changes: ProjectGitWorkingTreeChange[]) {
  return changes
    .filter(
      (change) =>
        change.stage_status === "staged"
        || change.stage_status === "partially_staged",
    )
    .map((change) => {
      if (change.change_type === "renamed" && change.previous_path) {
        return `重命名 ${change.previous_path} -> ${change.path}`;
      }
      const label =
        change.change_type === "added"
          ? "新增"
          : change.change_type === "deleted"
            ? "删除"
            : change.change_type === "renamed"
              ? "重命名"
              : "修改";
      return `${label} ${change.path}`;
    });
}

function countStageableFiles(overview: TaskGitCommitOverview | null) {
  if (!overview) {
    return 0;
  }
  return overview.working_tree_changes.filter(
    (change) =>
      change.stage_status === "unstaged"
      || change.stage_status === "untracked"
      || change.stage_status === "partially_staged",
  ).length;
}

function countStagedFiles(overview: TaskGitCommitOverview | null) {
  if (!overview) {
    return 0;
  }
  return overview.working_tree_changes.filter(
    (change) =>
      change.stage_status === "staged"
      || change.stage_status === "partially_staged",
  ).length;
}

export function TaskGitCommitDialog({
  open,
  onOpenChange,
  task,
  gitContext,
  onCommitted,
}: TaskGitCommitDialogProps) {
  const [overview, setOverview] = useState<TaskGitCommitOverview | null>(null);
  const [loadingOverview, setLoadingOverview] = useState(false);
  const [stagingAll, setStagingAll] = useState(false);
  const [generatingCommitMessage, setGeneratingCommitMessage] = useState(false);
  const [committing, setCommitting] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");
  const [error, setError] = useState<string | null>(null);

  const stageableFileCount = useMemo(
    () => countStageableFiles(overview),
    [overview],
  );
  const stagedFileCount = useMemo(
    () => countStagedFiles(overview),
    [overview],
  );

  const refreshOverview = async () => {
    setLoadingOverview(true);
    try {
      const nextOverview = await getTaskGitCommitOverview(gitContext.id);
      setOverview(nextOverview);
      return nextOverview;
    } catch (refreshError) {
      const message = refreshError instanceof Error ? refreshError.message : String(refreshError);
      setError(message);
      return null;
    } finally {
      setLoadingOverview(false);
    }
  };

  const ensureStagedOverview = async () => {
    const currentOverview = overview ?? await refreshOverview();
    if (!currentOverview) {
      return null;
    }
    if (countStagedFiles(currentOverview) > 0) {
      return currentOverview;
    }
    if (!currentOverview.working_tree_summary) {
      setError("当前任务 worktree 没有可提交的改动。");
      return null;
    }

    setStagingAll(true);
    setError(null);
    try {
      await stageAllTaskGitFiles(gitContext.id);
      const stagedOverview = await getTaskGitCommitOverview(gitContext.id);
      setOverview(stagedOverview);
      return stagedOverview;
    } catch (stageError) {
      setError(stageError instanceof Error ? stageError.message : String(stageError));
      return null;
    } finally {
      setStagingAll(false);
    }
  };

  useEffect(() => {
    if (!open) {
      return;
    }
    setCommitMessage("");
    setError(null);
    void refreshOverview();
  }, [gitContext.id, open]);

  const handleGenerateCommitMessage = async () => {
    setError(null);
    const currentOverview = await ensureStagedOverview();
    if (!currentOverview) {
      return;
    }
    const stagedChangePrompts = buildStagedChangePrompts(
      currentOverview.working_tree_changes,
    );
    if (stagedChangePrompts.length === 0) {
      setError("当前没有已暂存文件，无法生成提交信息。");
      return;
    }

    setGeneratingCommitMessage(true);
    try {
      const result = await aiGenerateCommitMessage(
        task.project_id,
        currentOverview.current_branch,
        currentOverview.working_tree_summary,
        stagedChangePrompts,
      );
      setCommitMessage(result);
    } catch (generateError) {
      setError(generateError instanceof Error ? generateError.message : String(generateError));
    } finally {
      setGeneratingCommitMessage(false);
    }
  };

  const handleSubmit = async () => {
    if (!commitMessage.trim()) {
      setError("提交说明不能为空。");
      return;
    }
    setError(null);
    const currentOverview = await ensureStagedOverview();
    if (!currentOverview) {
      return;
    }
    if (countStagedFiles(currentOverview) === 0) {
      setError("当前没有已暂存文件，无法创建提交。");
      return;
    }

    setCommitting(true);
    try {
      const result = await commitTaskGitChanges(gitContext.id, commitMessage.trim());
      await onCommitted?.(result);
      onOpenChange(false);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    } finally {
      setCommitting(false);
    }
  };

  const busy =
    loadingOverview || stagingAll || generatingCommitMessage || committing;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>提交任务代码</DialogTitle>
          <DialogDescription>
            基于任务 worktree 创建本地提交；如果当前还没有已暂存文件，会先自动暂存全部修改。
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>当前分支：{overview?.current_branch ?? gitContext.task_branch ?? "未知"}</div>
          <div className="mt-1">已暂存文件：{stagedFileCount}</div>
          <div className="mt-1">待暂存文件：{stageableFileCount}</div>
        </div>

        <div className="space-y-1.5">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs font-medium text-muted-foreground">提交说明</span>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => void handleGenerateCommitMessage()}
              disabled={busy || loadingOverview}
              title="AI 生成提交信息"
            >
              {generatingCommitMessage || stagingAll ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Sparkles className="h-4 w-4" />
              )}
            </Button>
          </div>
          <Textarea
            value={commitMessage}
            onChange={(event) => setCommitMessage(event.target.value)}
            disabled={busy}
            placeholder="输入提交信息…（Cmd/Ctrl+Enter 提交）"
            className="min-h-28"
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                void handleSubmit();
              }
            }}
          />
        </div>

        {error && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        <DialogFooter className="gap-2 sm:justify-between">
          <Button
            type="button"
            variant="ghost"
            onClick={() => void refreshOverview()}
            disabled={busy}
          >
            {loadingOverview ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                刷新中...
              </>
            ) : "刷新状态"}
          </Button>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={busy}
            >
              取消
            </Button>
            <Button type="button" onClick={() => void handleSubmit()} disabled={busy}>
              {committing ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  提交中...
                </>
              ) : stagingAll ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  暂存中...
                </>
              ) : "创建提交"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
