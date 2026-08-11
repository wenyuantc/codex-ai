import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { commitProjectGitChanges, pullProjectGitBranch, pushProjectGitBranch } from "@/lib/backend";
import { aiGenerateCommitMessage } from "@/lib/codex";
import { buildGitCommitChangePrompts } from "@/lib/gitWorkingTree";
import type { ProjectGitRepoActionType, ProjectGitWorkingTreeChange } from "@/lib/types";
import i18n from "@/lib/i18n";
import { GitCommitDialogContent } from "@/components/git/GitCommitDialogContent";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ProjectGitRepoActionDialogProps {
  open: boolean;
  action: ProjectGitRepoActionType | null;
  projectId: string | null;
  currentBranch?: string | null;
  workingTreeSummary?: string | null;
  projectBranches: string[];
  stagedFileCount: number;
  stagedChanges: ProjectGitWorkingTreeChange[];
  onOpenChange: (open: boolean) => void;
  onActionCompleted?: (message: string) => Promise<void> | void;
}

type PushForceMode = "none" | "force" | "force_with_lease";
type PullMode = "ff_only" | "rebase";

const PUSH_FORCE_MODE_OPTIONS: Array<{ value: PushForceMode; labelKey: string }> = [
  { value: "none", labelKey: "projects:gitForceModes.none" },
  { value: "force", labelKey: "projects:gitForceModes.force" },
  { value: "force_with_lease", labelKey: "projects:gitForceModes.force_with_lease" },
];

const PULL_MODE_OPTIONS: Array<{ value: PullMode; labelKey: string }> = [
  { value: "ff_only", labelKey: "projects:gitRepoDialog.pullModes.ff_only" },
  { value: "rebase", labelKey: "projects:gitRepoDialog.pullModes.rebase" },
];

function getDialogTitle(action: ProjectGitRepoActionType | null) {
  const key =
    action === "commit" || action === "push" || action === "pull"
      ? `projects:gitRepoDialog.${action}`
      : "projects:gitRepoDialog.title";
  return i18n.t(key);
}

function getDialogDescription(action: ProjectGitRepoActionType | null, stagedFileCount: number) {
  switch (action) {
    case "commit":
      return stagedFileCount > 0
        ? i18n.t("projects:gitRepoDialog.commitDescriptionStaged", { count: stagedFileCount })
        : i18n.t("projects:gitRepoDialog.commitDescriptionEmpty");
    case "push":
      return i18n.t("projects:gitRepoDialog.pushDescription");
    case "pull":
      return i18n.t("projects:gitRepoDialog.pullDescription");
    default:
      return i18n.t("projects:gitRepoDialog.defaultDescription");
  }
}

function getSubmitLabel(action: ProjectGitRepoActionType | null, submitting: boolean) {
  if (submitting) {
    const key =
      action === "commit"
        ? "projects:gitRepoDialog.committing"
        : action === "push"
          ? "projects:gitRepoDialog.pushing"
          : action === "pull"
            ? "projects:gitRepoDialog.pulling"
            : "projects:gitActionDialog.executing";
    return i18n.t(key);
  }

  const key =
    action === "commit"
      ? "projects:gitRepoDialog.createCommit"
      : action === "push"
        ? "projects:gitRepoDialog.pushNow"
        : action === "pull"
          ? "projects:gitRepoDialog.pullNow"
          : "projects:gitBranchDialog.execute";
  return i18n.t(key);
}

function getPullModeLabel(pullMode: PullMode) {
  const option = PULL_MODE_OPTIONS.find((item) => item.value === pullMode);
  return option ? i18n.t(option.labelKey) : pullMode;
}

function getPushForceModeLabel(forceMode: PushForceMode) {
  const option = PUSH_FORCE_MODE_OPTIONS.find((item) => item.value === forceMode);
  return option ? i18n.t(option.labelKey) : forceMode;
}

function buildBranchOptions(
  currentBranch: string | null | undefined,
  projectBranches: string[],
  currentValue: string,
) {
  const candidates = [currentValue, currentBranch ?? "", ...projectBranches];
  const seen = new Set<string>();

  return candidates
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
    .filter((value) => {
      if (seen.has(value)) {
        return false;
      }
      seen.add(value);
      return true;
    })
    .map((value) => ({ value, label: value }));
}

export function ProjectGitRepoActionDialog({
  open,
  action,
  projectId,
  currentBranch,
  workingTreeSummary,
  projectBranches,
  stagedFileCount,
  stagedChanges,
  onOpenChange,
  onActionCompleted,
}: ProjectGitRepoActionDialogProps) {
  const { t } = useTranslation(["projects", "common"]);
  const [commitMessage, setCommitMessage] = useState("");
  const [remoteName, setRemoteName] = useState("origin");
  const [branchName, setBranchName] = useState("");
  const [pushForceMode, setPushForceMode] = useState<PushForceMode>("none");
  const [pullMode, setPullMode] = useState<PullMode>("ff_only");
  const [pullAutoStash, setPullAutoStash] = useState(false);
  const [submitMode, setSubmitMode] = useState<"primary" | "commit_push" | null>(null);
  const [generatingCommitMessage, setGeneratingCommitMessage] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasWorkingTreeChanges = Boolean(workingTreeSummary);
  const submitting = submitMode !== null;

  useEffect(() => {
    if (!open) {
      return;
    }

    setCommitMessage("");
    setRemoteName("origin");
    setBranchName(currentBranch ?? projectBranches[0] ?? "");
    setPushForceMode("none");
    setPullMode(workingTreeSummary ? "rebase" : "ff_only");
    setPullAutoStash(Boolean(workingTreeSummary));
    setError(null);
  }, [action, currentBranch, open, projectBranches, workingTreeSummary]);

  const branchOptions = buildBranchOptions(currentBranch, projectBranches, branchName);

  const stagedChangePrompts = buildGitCommitChangePrompts(stagedChanges);

  const handleGenerateCommitMessage = async () => {
    if (!projectId) {
      setError(t("gitRepoDialog.generateErrorNoProject"));
      return;
    }
    if (stagedChangePrompts.length === 0) {
      setError(t("gitRepoDialog.generateErrorNoStaged"));
      return;
    }

    setGeneratingCommitMessage(true);
    setError(null);
    try {
      const result = await aiGenerateCommitMessage(
        projectId,
        currentBranch ?? null,
        workingTreeSummary ?? null,
        stagedChangePrompts,
      );
      setCommitMessage(result);
    } catch (generateError) {
      setError(generateError instanceof Error ? generateError.message : String(generateError));
    } finally {
      setGeneratingCommitMessage(false);
    }
  };

  const handleSubmit = async (mode: "primary" | "commit_push" = "primary") => {
    if (!projectId || !action) {
      return;
    }

    if (action === "commit") {
      if (stagedFileCount === 0) {
        setError(t("gitRepoDialog.commitNoStagedError"));
        return;
      }
      if (!commitMessage.trim()) {
        setError(t("gitRepoDialog.commitMessageEmptyError"));
        return;
      }
    }

    setSubmitMode(mode);
    setError(null);
    try {
      let result: string;
      if (action === "commit") {
        const commitResult = await commitProjectGitChanges(projectId, commitMessage.trim());
        if (mode === "commit_push") {
          const branch = currentBranch?.trim();
          if (!branch) {
            await onActionCompleted?.(commitResult);
            setError(t("gitRepoDialog.commitPushUnknownBranch"));
            return;
          }
          try {
            const pushResult = await pushProjectGitBranch(projectId, "origin", branch, "none");
            result = `${commitResult}\n${pushResult}`;
          } catch (pushError) {
            await onActionCompleted?.(commitResult);
            setError(
              t("gitRepoDialog.commitPushFailed", {
                message: pushError instanceof Error ? pushError.message : String(pushError),
              }),
            );
            return;
          }
        } else {
          result = commitResult;
        }
      } else if (action === "push") {
        result = await pushProjectGitBranch(
          projectId,
          remoteName.trim(),
          branchName.trim(),
          pushForceMode,
        );
      } else {
        result = await pullProjectGitBranch(
          projectId,
          remoteName.trim(),
          branchName.trim(),
          pullMode,
          pullAutoStash,
        );
      }
      await onActionCompleted?.(result);
      onOpenChange(false);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    } finally {
      setSubmitMode(null);
    }
  };

  const commitDisabled = action === "commit" && stagedFileCount === 0;
  const shouldRenderDialog = open && Boolean(projectId) && action !== null;

  if (!shouldRenderDialog) {
    return null;
  }

  if (action === "commit") {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <GitCommitDialogContent
          title={getDialogTitle(action)}
          description={getDialogDescription(action, stagedFileCount)}
          summaryRows={[
            {
              label: t("gitRepoDialog.currentBranchLabel"),
              value: currentBranch ?? t("common:unknown"),
            },
            { label: t("gitRepoDialog.stagedFilesLabel"), value: stagedFileCount },
          ]}
          commitMessage={commitMessage}
          busy={submitting || generatingCommitMessage}
          generatingCommitMessage={generatingCommitMessage}
          error={error}
          generateDisabled={stagedFileCount === 0}
          submitDisabled={commitDisabled}
          submitLabel={getSubmitLabel(action, submitMode === "primary")}
          onCommitMessageChange={setCommitMessage}
          onGenerateCommitMessage={handleGenerateCommitMessage}
          onCancel={() => onOpenChange(false)}
          onSubmit={() => handleSubmit("primary")}
          extraActions={
            <Button
              type="button"
              variant="outline"
              onClick={() => void handleSubmit("commit_push")}
              disabled={submitting || commitDisabled || !currentBranch}
              title={
                currentBranch
                  ? t("gitRepoDialog.pushAfterCommitTitle", { branch: currentBranch })
                  : t("gitRepoDialog.pushAfterCommitUnknownTitle")
              }
            >
              {submitMode === "commit_push"
                ? t("gitRepoDialog.commitPushSubmitting")
                : t("gitRepoDialog.commitPush")}
            </Button>
          }
        />
      </Dialog>
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{getDialogTitle(action)}</DialogTitle>
          <DialogDescription>{getDialogDescription(action, stagedFileCount)}</DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>
            {t("gitRepoDialog.currentBranchLabel")}：{currentBranch ?? t("common:unknown")}
          </div>
          <div className="mt-1">
            {t("gitRepoDialog.stagedFilesLabel")}：{stagedFileCount}
          </div>
        </div>

        {action === "push" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.remoteName")}
              </span>
              <Input
                value={remoteName}
                onChange={(event) => setRemoteName(event.target.value)}
                disabled={submitting}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitRepoDialog.branchName")}
              </span>
              <Select<string>
                value={branchName}
                onValueChange={(value) => {
                  if (value) {
                    setBranchName(value);
                  }
                }}
                disabled={submitting || branchOptions.length === 0}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{branchName || t("gitBranchDialog.selectBranch")}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {branchOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="space-y-1.5 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.pushMode")}
              </span>
              <Select<PushForceMode>
                value={pushForceMode}
                onValueChange={(value) => {
                  if (value) {
                    setPushForceMode(value);
                  }
                }}
                disabled={submitting}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{getPushForceModeLabel(pushForceMode)}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {PUSH_FORCE_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {i18n.t(option.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
          </div>
        ) : action === "pull" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.remoteName")}
              </span>
              <Input
                value={remoteName}
                onChange={(event) => setRemoteName(event.target.value)}
                disabled={submitting}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitRepoDialog.branchName")}
              </span>
              <Select<string>
                value={branchName}
                onValueChange={(value) => {
                  if (value) {
                    setBranchName(value);
                  }
                }}
                disabled={submitting || branchOptions.length === 0}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{branchName || t("gitBranchDialog.selectBranch")}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {branchOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="space-y-1.5 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitRepoDialog.pullMode")}
              </span>
              <Select<PullMode>
                value={pullMode}
                onValueChange={(value) => {
                  if (value) {
                    setPullMode(value);
                  }
                }}
                disabled={submitting}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{getPullModeLabel(pullMode)}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {PULL_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {i18n.t(option.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm sm:col-span-2">
              <input
                type="checkbox"
                checked={pullAutoStash}
                onChange={(event) => setPullAutoStash(event.target.checked)}
                disabled={submitting}
              />
              {t("gitRepoDialog.pullAutostash")}
            </label>
            {hasWorkingTreeChanges && (
              <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900 sm:col-span-2">
                {pullMode === "rebase"
                  ? t("gitRepoDialog.pullRebaseHint")
                  : t("gitRepoDialog.pullFfHint")}
              </div>
            )}
          </div>
        ) : null}

        {branchOptions.length === 0 && (
          <div className="rounded-md border border-dashed border-border px-3 py-2 text-xs text-muted-foreground">
            {t("gitRepoDialog.noBranchesHint")}
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
          <Button
            type="button"
            onClick={() => void handleSubmit("primary")}
            disabled={submitting || !action || commitDisabled}
          >
            {getSubmitLabel(action, submitMode === "primary")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
