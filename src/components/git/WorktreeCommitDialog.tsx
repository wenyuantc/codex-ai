import { useEffect, useMemo, useState } from "react";

import {
  commitProjectWorktreeChanges,
  generateProjectWorktreeCommitMessage,
} from "@/lib/backend";
import {
  countStageableGitFiles,
  countStagedGitFiles,
} from "@/lib/gitWorkingTree";
import type { ProjectGitWorktree } from "@/lib/types";
import { GitCommitDialogContent } from "@/components/git/GitCommitDialogContent";
import { Dialog } from "@/components/ui/dialog";

interface WorktreeCommitDialogProps {
  open: boolean;
  projectId: string;
  worktree: ProjectGitWorktree | null;
  onOpenChange: (open: boolean) => void;
  onCommitted?: (message: string) => Promise<void> | void;
}

export function WorktreeCommitDialog({
  open,
  projectId,
  worktree,
  onOpenChange,
  onCommitted,
}: WorktreeCommitDialogProps) {
  const [commitMessage, setCommitMessage] = useState("");
  const [generatingCommitMessage, setGeneratingCommitMessage] = useState(false);
  const [committing, setCommitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const stageableFileCount = useMemo(
    () => countStageableGitFiles(worktree?.working_tree_changes ?? []),
    [worktree],
  );
  const stagedFileCount = useMemo(
    () => countStagedGitFiles(worktree?.working_tree_changes ?? []),
    [worktree],
  );
  const busy = generatingCommitMessage || committing;

  useEffect(() => {
    if (!open) {
      return;
    }
    setCommitMessage("");
    setError(null);
  }, [open, worktree?.path]);

  const handleGenerateCommitMessage = async () => {
    if (!worktree) {
      setError("当前 worktree 不存在，无法生成提交信息。");
      return;
    }
    if (stagedFileCount === 0) {
      setError("当前没有已暂存文件，无法生成提交信息。");
      return;
    }

    setGeneratingCommitMessage(true);
    setError(null);
    try {
      const message = await generateProjectWorktreeCommitMessage(projectId, worktree.path);
      setCommitMessage(message);
    } catch (generateError) {
      setError(generateError instanceof Error ? generateError.message : String(generateError));
    } finally {
      setGeneratingCommitMessage(false);
    }
  };

  const handleSubmit = async () => {
    if (!worktree) {
      setError("当前 worktree 不存在，无法提交。");
      return;
    }
    if (stagedFileCount === 0) {
      setError("当前没有已暂存文件，请先暂存后再提交。");
      return;
    }
    if (!commitMessage.trim()) {
      setError("提交说明不能为空。");
      return;
    }

    setCommitting(true);
    setError(null);
    try {
      const result = await commitProjectWorktreeChanges(projectId, worktree.path, commitMessage.trim());
      await onCommitted?.(result);
      onOpenChange(false);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    } finally {
      setCommitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GitCommitDialogContent
        title="提交 Worktree 改动"
        description="基于当前 worktree 已暂存的文件创建本地提交。"
        summaryRows={[
          { label: "当前分支", value: worktree?.branch ?? (worktree?.is_detached ? "detached HEAD" : "未知") },
          { label: "已暂存文件", value: stagedFileCount },
          { label: "待暂存文件", value: stageableFileCount },
          { label: "Worktree", value: <span className="break-all font-mono">{worktree?.path ?? "未知"}</span> },
        ]}
        commitMessage={commitMessage}
        busy={busy}
        generatingCommitMessage={generatingCommitMessage}
        error={error}
        generateDisabled={!worktree || stagedFileCount === 0}
        submitDisabled={!worktree || stagedFileCount === 0}
        submitLabel={committing ? "提交中..." : "创建提交"}
        onCommitMessageChange={setCommitMessage}
        onGenerateCommitMessage={handleGenerateCommitMessage}
        onCancel={() => onOpenChange(false)}
        onSubmit={handleSubmit}
      />
    </Dialog>
  );
}
