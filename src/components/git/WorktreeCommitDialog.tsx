import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { commitProjectWorktreeChanges, generateProjectWorktreeCommitMessage } from "@/lib/backend";
import { countStageableGitFiles, countStagedGitFiles } from "@/lib/gitWorkingTree";
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
  const { t } = useTranslation("projects");
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
      setError(t("worktreeCommitDialog.generateErrorNoWorktree"));
      return;
    }
    if (stagedFileCount === 0) {
      setError(t("worktreeCommitDialog.generateErrorNoStaged"));
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
      setError(t("worktreeCommitDialog.commitDisabled"));
      return;
    }
    if (stagedFileCount === 0) {
      setError(t("worktreeCommitDialog.commitNoStagedError"));
      return;
    }
    if (!commitMessage.trim()) {
      setError(t("worktreeCommitDialog.commitMessageEmptyError"));
      return;
    }

    setCommitting(true);
    setError(null);
    try {
      const result = await commitProjectWorktreeChanges(
        projectId,
        worktree.path,
        commitMessage.trim(),
      );
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
        title={t("worktreeCommitDialog.title")}
        description={t("worktreeCommitDialog.description")}
        summaryRows={[
          {
            label: t("worktreeCommitDialog.currentBranchLabel"),
            value:
              worktree?.branch ??
              (worktree?.is_detached ? "detached HEAD" : t("worktree.unknownBranch")),
          },
          { label: t("worktreeCommitDialog.stagedFilesLabel"), value: stagedFileCount },
          { label: t("worktreeCommitDialog.stageableFilesLabel"), value: stageableFileCount },
          {
            label: t("worktreeCommitDialog.worktreeLabel"),
            value: (
              <span className="break-all font-mono">
                {worktree?.path ?? t("worktree.unknownBranch")}
              </span>
            ),
          },
        ]}
        commitMessage={commitMessage}
        busy={busy}
        generatingCommitMessage={generatingCommitMessage}
        error={error}
        generateDisabled={!worktree || stagedFileCount === 0}
        submitDisabled={!worktree || stagedFileCount === 0}
        submitLabel={committing ? t("gitRepoDialog.committing") : t("gitRepoDialog.createCommit")}
        onCommitMessageChange={setCommitMessage}
        onGenerateCommitMessage={handleGenerateCommitMessage}
        onCancel={() => onOpenChange(false)}
        onSubmit={handleSubmit}
      />
    </Dialog>
  );
}
