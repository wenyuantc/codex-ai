import { useEffect, useMemo, useState } from "react";

import {
  commitTaskGitChanges,
  getTaskGitCommitOverview,
  stageAllTaskGitFiles,
} from "@/lib/backend";
import { aiGenerateCommitMessage } from "@/lib/codex";
import {
  buildGitCommitChangePrompts,
  countStageableGitFiles,
  countStagedGitFiles,
} from "@/lib/gitWorkingTree";
import type {
  Task,
  TaskGitCommitOverview,
  TaskGitContext,
} from "@/lib/types";
import { GitCommitDialogContent } from "@/components/git/GitCommitDialogContent";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Loader2 } from "lucide-react";

interface TaskGitCommitDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  task: Task;
  gitContext: TaskGitContext;
  initialOverview?: TaskGitCommitOverview | null;
  initialError?: string | null;
  onCommitted?: (message: string) => Promise<void> | void;
}

export function TaskGitCommitDialog({
  open,
  onOpenChange,
  task,
  gitContext,
  initialOverview = null,
  initialError = null,
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
    () => countStageableGitFiles(overview?.working_tree_changes ?? []),
    [overview],
  );
  const stagedFileCount = useMemo(
    () => countStagedGitFiles(overview?.working_tree_changes ?? []),
    [overview],
  );

  const refreshOverview = async () => {
    setLoadingOverview(true);
    try {
      const nextOverview = await getTaskGitCommitOverview(gitContext.id);
      setOverview(nextOverview);
      setError(null);
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
    if (countStagedGitFiles(currentOverview.working_tree_changes) > 0) {
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
    setOverview(initialOverview);
    setError(initialError);
    if (!initialOverview && !initialError) {
      void refreshOverview();
    }
  }, [gitContext.id, initialError, initialOverview, open]);

  const handleGenerateCommitMessage = async () => {
    setError(null);
    const currentOverview = await ensureStagedOverview();
    if (!currentOverview) {
      return;
    }
    const stagedChangePrompts = buildGitCommitChangePrompts(
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
    if (countStagedGitFiles(currentOverview.working_tree_changes) === 0) {
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
      <GitCommitDialogContent
        title="提交任务代码"
        description="基于任务 worktree 创建本地提交；如果当前还没有已暂存文件，会先自动暂存全部修改。"
        summaryRows={[
          { label: "当前分支", value: overview?.current_branch ?? gitContext.task_branch ?? "未知" },
          { label: "已暂存文件", value: stagedFileCount },
          { label: "待暂存文件", value: stageableFileCount },
        ]}
        commitMessage={commitMessage}
        busy={busy}
        generatingCommitMessage={generatingCommitMessage || stagingAll}
        error={error}
        generateDisabled={loadingOverview}
        submitLabel={
          committing
            ? "提交中..."
            : stagingAll
              ? "暂存中..."
              : "创建提交"
        }
        onCommitMessageChange={setCommitMessage}
        onGenerateCommitMessage={handleGenerateCommitMessage}
        onCancel={() => onOpenChange(false)}
        onSubmit={handleSubmit}
        footerStart={(
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
        )}
      />
    </Dialog>
  );
}
