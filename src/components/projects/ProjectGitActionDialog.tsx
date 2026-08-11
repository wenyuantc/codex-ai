import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { cancelGitAction, confirmGitAction, requestGitAction } from "@/lib/backend";
import type { GitActionType, TaskGitContext } from "@/lib/types";
import i18n from "@/lib/i18n";
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
import { Textarea } from "@/components/ui/textarea";

interface ProjectGitActionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  context: TaskGitContext | null;
  projectBranches: string[];
  preferredAction?: GitActionType | null;
  lockActionSelection?: boolean;
  onActionStateChanged?: () => Promise<void> | void;
  onActionCompleted?: (message: string) => Promise<void> | void;
}

interface GitActionFormState {
  targetBranch: string;
  strategy: string;
  allowFastForward: boolean;
  remoteName: string;
  sourceBranch: string;
  targetRef: string;
  forceMode: "none" | "force" | "force_with_lease";
  ontoBranch: string;
  autoStash: boolean;
  cherryPickCommitIds: string;
  includeUntracked: boolean;
  stashMessage: string;
  stashRef: string;
  deleteBranch: boolean;
  pruneWorktree: boolean;
}

const GIT_ACTION_OPTIONS: Array<{ value: GitActionType; labelKey: string }> = [
  { value: "merge", labelKey: "projects:gitActionOptions.merge" },
  { value: "push", labelKey: "projects:gitActionOptions.push" },
  { value: "rebase", labelKey: "projects:gitActionOptions.rebase" },
  { value: "cherry_pick", labelKey: "projects:gitActionOptions.cherry_pick" },
  { value: "stash", labelKey: "projects:gitActionOptions.stash" },
  { value: "unstash", labelKey: "projects:gitActionOptions.unstash" },
  { value: "cleanup_worktree", labelKey: "projects:gitActionOptions.cleanup_worktree" },
];

const FORCE_MODE_OPTIONS: Array<{
  value: GitActionFormState["forceMode"];
  labelKey: string;
}> = [
  { value: "none", labelKey: "projects:gitForceModes.none" },
  { value: "force", labelKey: "projects:gitForceModes.force" },
  { value: "force_with_lease", labelKey: "projects:gitForceModes.force_with_lease" },
];

const MERGE_STRATEGY_OPTIONS: Array<{ value: string; labelKey: string }> = [
  { value: "ort", labelKey: "projects:gitMergeStrategies.ort" },
  { value: "recursive", labelKey: "projects:gitMergeStrategies.recursive" },
  { value: "resolve", labelKey: "projects:gitMergeStrategies.resolve" },
  { value: "ours", labelKey: "projects:gitMergeStrategies.ours" },
  { value: "subtree", labelKey: "projects:gitMergeStrategies.subtree" },
];

function getTaskGitContextStateLabel(state: string) {
  return i18n.t(`projects:gitContextState.${state}`, { defaultValue: state });
}

function getGitActionLabel(actionType: GitActionType) {
  const option = GIT_ACTION_OPTIONS.find((item) => item.value === actionType);
  return option ? i18n.t(option.labelKey) : actionType;
}

function getMergeStrategyLabel(strategy: string) {
  const option = MERGE_STRATEGY_OPTIONS.find((item) => item.value === strategy);
  return option ? i18n.t(option.labelKey) : strategy;
}

function getForceModeLabel(forceMode: GitActionFormState["forceMode"]) {
  const option = FORCE_MODE_OPTIONS.find((item) => item.value === forceMode);
  return option ? i18n.t(option.labelKey) : forceMode;
}

function buildBranchOptions(
  projectBranches: string[],
  context: TaskGitContext | null,
  currentValue: string,
) {
  const candidates = [
    currentValue,
    context?.target_branch ?? "",
    context?.base_branch ?? "",
    ...projectBranches,
  ];
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

function buildInitialFormState(context: TaskGitContext | null): GitActionFormState {
  let payload: Record<string, unknown> | null = null;
  if (typeof context?.pending_action_payload_json === "string") {
    try {
      payload = JSON.parse(context.pending_action_payload_json) as Record<string, unknown>;
    } catch {
      payload = null;
    }
  }

  return {
    targetBranch:
      typeof payload?.target_branch === "string"
        ? payload.target_branch
        : (context?.target_branch ?? ""),
    strategy: typeof payload?.strategy === "string" ? payload.strategy : "ort",
    allowFastForward: typeof payload?.allow_ff === "boolean" ? payload.allow_ff : true,
    remoteName: typeof payload?.remote_name === "string" ? payload.remote_name : "origin",
    sourceBranch:
      typeof payload?.source_branch === "string"
        ? payload.source_branch
        : (context?.task_branch ?? ""),
    targetRef:
      typeof payload?.target_ref === "string" ? payload.target_ref : (context?.task_branch ?? ""),
    forceMode:
      payload?.force_mode === "force" || payload?.force_mode === "force_with_lease"
        ? payload.force_mode
        : "none",
    ontoBranch:
      typeof payload?.onto_branch === "string"
        ? payload.onto_branch
        : (context?.target_branch ?? ""),
    autoStash: typeof payload?.auto_stash === "boolean" ? payload.auto_stash : false,
    cherryPickCommitIds: Array.isArray(payload?.commit_ids)
      ? payload.commit_ids.filter((item): item is string => typeof item === "string").join("\n")
      : "",
    includeUntracked:
      typeof payload?.include_untracked === "boolean" ? payload.include_untracked : false,
    stashMessage: typeof payload?.message === "string" ? payload.message : "",
    stashRef: typeof payload?.stash_ref === "string" ? payload.stash_ref : "stash@{0}",
    deleteBranch: typeof payload?.delete_branch === "boolean" ? payload.delete_branch : false,
    pruneWorktree: typeof payload?.prune_worktree === "boolean" ? payload.prune_worktree : true,
  };
}

export function ProjectGitActionDialog({
  open,
  onOpenChange,
  context,
  projectBranches,
  preferredAction,
  lockActionSelection = false,
  onActionStateChanged,
  onActionCompleted,
}: ProjectGitActionDialogProps) {
  const { t } = useTranslation("projects");
  const [selectedAction, setSelectedAction] = useState<GitActionType>("merge");
  const [form, setForm] = useState<GitActionFormState>(() => buildInitialFormState(null));
  const [pendingToken, setPendingToken] = useState<string | null>(null);
  const [pendingExpiresAt, setPendingExpiresAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [requesting, setRequesting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }

    const initialAction =
      preferredAction ??
      context?.pending_action_type ??
      (context?.state === "drifted" ? "cleanup_worktree" : "merge");
    setSelectedAction(initialAction);
    setForm(buildInitialFormState(context));
    setPendingToken(null);
    setPendingExpiresAt(null);
    setError(null);
    setInfo(null);
  }, [context, open, preferredAction]);

  const hasExistingPendingAction = Boolean(context?.pending_action_type);
  const formLocked = pendingToken !== null || requesting || confirming || cancelling;
  const actionSummary = context
    ? [
        t("gitActionDialog.taskBranch", {
          branch: context.task_branch ?? t("unnamedBranch"),
        }),
        t("targetBranch", { branch: context.target_branch ?? t("notSet") }),
        t("gitActionDialog.currentState", {
          state: getTaskGitContextStateLabel(context.state),
        }),
      ].join(" · ")
    : null;
  const mergeTargetBranchOptions = buildBranchOptions(projectBranches, context, form.targetBranch);
  const rebaseTargetBranchOptions = buildBranchOptions(projectBranches, context, form.ontoBranch);

  const updateForm = (patch: Partial<GitActionFormState>) => {
    setForm((current) => ({ ...current, ...patch }));
  };

  const buildPayload = () => {
    switch (selectedAction) {
      case "merge":
        return {
          target_branch: form.targetBranch,
          strategy: form.strategy,
          allow_ff: form.allowFastForward,
        };
      case "push":
        return {
          remote_name: form.remoteName,
          source_branch: form.sourceBranch,
          target_ref: form.targetRef,
          force_mode: form.forceMode,
        };
      case "rebase":
        return {
          onto_branch: form.ontoBranch,
          auto_stash: form.autoStash,
        };
      case "cherry_pick":
        return {
          commit_ids: form.cherryPickCommitIds
            .split("\n")
            .map((item) => item.trim())
            .filter(Boolean),
        };
      case "stash":
        return {
          include_untracked: form.includeUntracked,
          message: form.stashMessage || null,
        };
      case "unstash":
        return {
          stash_ref: form.stashRef,
        };
      case "cleanup_worktree":
        return {
          delete_branch: form.deleteBranch,
          prune_worktree: form.pruneWorktree,
        };
    }
  };

  const handleRequest = async () => {
    if (!context) {
      return;
    }

    setRequesting(true);
    setError(null);
    setInfo(null);
    try {
      const result = await requestGitAction(context.id, selectedAction, buildPayload());
      setPendingToken(result.token);
      setPendingExpiresAt(result.expires_at);
      setInfo(t("gitActionDialog.requestInfo", { action: getGitActionLabel(selectedAction) }));
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : String(requestError));
    } finally {
      setRequesting(false);
    }
  };

  const handleConfirm = async () => {
    if (!context || !pendingToken) {
      return;
    }

    setConfirming(true);
    setError(null);
    try {
      const result = await confirmGitAction(context.id, pendingToken);
      await onActionCompleted?.(result.message);
      onOpenChange(false);
    } catch (confirmError) {
      setError(confirmError instanceof Error ? confirmError.message : String(confirmError));
      await onActionStateChanged?.();
    } finally {
      setConfirming(false);
    }
  };

  const handleCancel = async () => {
    if (!context) {
      return;
    }

    setCancelling(true);
    setError(null);
    try {
      await cancelGitAction(context.id, pendingToken ?? undefined);
      await onActionStateChanged?.();
      await onActionCompleted?.(
        pendingToken
          ? t("gitActionDialog.cancelledGate", { action: getGitActionLabel(selectedAction) })
          : t("gitActionDialog.cancelledPending"),
      );
      onOpenChange(false);
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : String(cancelError));
    } finally {
      setCancelling(false);
    }
  };

  const renderActionFields = () => {
    switch (selectedAction) {
      case "merge":
        return (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.targetBranchField")}
              </span>
              <Select<string>
                value={form.targetBranch}
                onValueChange={(value) => {
                  if (value) {
                    updateForm({ targetBranch: value });
                  }
                }}
                disabled={formLocked}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>
                    {form.targetBranch || t("gitActionDialog.selectTargetBranch")}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {mergeTargetBranchOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.mergeStrategyField")}
              </span>
              <Select<string>
                value={form.strategy}
                onValueChange={(value) => {
                  if (value) {
                    updateForm({ strategy: value });
                  }
                }}
                disabled={formLocked}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{getMergeStrategyLabel(form.strategy)}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {MERGE_STRATEGY_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {i18n.t(option.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="col-span-full flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.allowFastForward}
                onChange={(event) => updateForm({ allowFastForward: event.target.checked })}
                disabled={formLocked}
              />
              {t("gitActionDialog.allowFastForward")}
            </label>
          </div>
        );
      case "push":
        return (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.remoteName")}
              </span>
              <Input
                value={form.remoteName}
                onChange={(event) => updateForm({ remoteName: event.target.value })}
                disabled={formLocked}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.sourceBranch")}
              </span>
              <Input
                value={form.sourceBranch}
                onChange={(event) => updateForm({ sourceBranch: event.target.value })}
                disabled={formLocked}
                placeholder="codex/task-..."
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.targetRef")}
              </span>
              <Input
                value={form.targetRef}
                onChange={(event) => updateForm({ targetRef: event.target.value })}
                disabled={formLocked}
                placeholder="codex/task-..."
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.pushMode")}
              </span>
              <Select<GitActionFormState["forceMode"]>
                value={form.forceMode}
                onValueChange={(value) => {
                  if (value) {
                    updateForm({ forceMode: value });
                  }
                }}
                disabled={formLocked}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>{getForceModeLabel(form.forceMode)}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {FORCE_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {i18n.t(option.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
          </div>
        );
      case "rebase":
        return (
          <div className="grid gap-3">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.targetBranchField")}
              </span>
              <Select<string>
                value={form.ontoBranch}
                onValueChange={(value) => {
                  if (value) {
                    updateForm({ ontoBranch: value });
                  }
                }}
                disabled={formLocked}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>
                    {form.ontoBranch || t("gitActionDialog.selectTargetBranch")}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {rebaseTargetBranchOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.autoStash}
                onChange={(event) => updateForm({ autoStash: event.target.checked })}
                disabled={formLocked}
              />
              {t("gitActionDialog.rebaseAutoStash")}
            </label>
          </div>
        );
      case "cherry_pick":
        return (
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {t("gitActionDialog.cherryPickSha")}
            </span>
            <Textarea
              value={form.cherryPickCommitIds}
              onChange={(event) => updateForm({ cherryPickCommitIds: event.target.value })}
              disabled={formLocked}
              placeholder={t("gitActionDialog.cherryPickPlaceholder")}
              className="min-h-24"
            />
          </label>
        );
      case "stash":
        return (
          <div className="grid gap-3">
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.includeUntracked}
                onChange={(event) => updateForm({ includeUntracked: event.target.checked })}
                disabled={formLocked}
              />
              {t("gitActionDialog.includeUntracked")}
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                {t("gitActionDialog.stashMessage")}
              </span>
              <Input
                value={form.stashMessage}
                onChange={(event) => updateForm({ stashMessage: event.target.value })}
                disabled={formLocked}
                placeholder={t("milestoneOptionalPlaceholder")}
              />
            </label>
          </div>
        );
      case "unstash":
        return (
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {t("gitActionDialog.stashRef")}
            </span>
            <Input
              value={form.stashRef}
              onChange={(event) => updateForm({ stashRef: event.target.value })}
              disabled={formLocked}
              placeholder="stash@{0}"
            />
          </label>
        );
      case "cleanup_worktree":
        return (
          <div className="grid gap-3">
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.deleteBranch}
                onChange={(event) => updateForm({ deleteBranch: event.target.checked })}
                disabled={formLocked}
              />
              {t("gitActionDialog.cleanupDeleteBranch")}
            </label>
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.pruneWorktree}
                onChange={(event) => updateForm({ pruneWorktree: event.target.checked })}
                disabled={formLocked}
              />
              {t("gitActionDialog.cleanupPruneWorktree")}
            </label>
          </div>
        );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,42rem)] max-w-[min(96vw,42rem)] sm:max-w-[min(96vw,42rem)]">
        <DialogHeader>
          <DialogTitle>{t("gitActionDialog.title")}</DialogTitle>
          <DialogDescription>{t("gitActionDialog.description")}</DialogDescription>
        </DialogHeader>

        {context ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-border/60 bg-secondary/20 px-3 py-2 text-xs text-muted-foreground">
              {actionSummary}
            </div>

            {hasExistingPendingAction && pendingToken === null && (
              <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
                {t("gitActionDialog.existingPendingHint", {
                  action: getGitActionLabel(context.pending_action_type ?? "merge"),
                })}
              </div>
            )}

            <div className="space-y-3">
              <label className="space-y-1.5">
                <span className="text-xs font-medium text-muted-foreground">
                  {t("gitActionDialog.actionType")}
                </span>
                <Select<GitActionType>
                  value={selectedAction}
                  onValueChange={(value) => {
                    if (value) {
                      setSelectedAction(value);
                    }
                  }}
                  disabled={formLocked || lockActionSelection}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue>{getGitActionLabel(selectedAction)}</SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {GIT_ACTION_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {i18n.t(option.labelKey)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
              {lockActionSelection && preferredAction ? (
                <div className="rounded-lg border border-border/60 bg-secondary/20 px-3 py-2 text-xs text-muted-foreground">
                  {t("gitActionDialog.lockedActionHint", {
                    action: getGitActionLabel(preferredAction),
                  })}
                </div>
              ) : null}

              {renderActionFields()}
            </div>

            {pendingToken ? (
              <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">
                  {t("gitActionDialog.gateGenerated")}
                </div>
                <div className="mt-1 break-all">
                  {t("gitActionDialog.token", { token: pendingToken })}
                </div>
                <div className="mt-1">
                  {t("gitActionDialog.expiresAt", {
                    time: pendingExpiresAt ?? t("common:unknown"),
                  })}
                </div>
              </div>
            ) : null}

            {info ? (
              <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-primary">
                {info}
              </div>
            ) : null}

            {error ? (
              <div className="rounded-lg border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {error}
              </div>
            ) : null}
          </div>
        ) : null}

        <DialogFooter>
          {pendingToken ? (
            <>
              <Button
                variant="outline"
                onClick={() => {
                  setPendingToken(null);
                  setPendingExpiresAt(null);
                  setInfo(null);
                  setError(null);
                }}
                disabled={confirming || cancelling}
              >
                {t("gitActionDialog.reselect")}
              </Button>
              <Button variant="outline" onClick={handleCancel} disabled={confirming || cancelling}>
                {cancelling ? t("gitActionDialog.cancelling") : t("gitActionDialog.cancelGate")}
              </Button>
              <Button onClick={handleConfirm} disabled={confirming || cancelling}>
                {confirming ? t("gitActionDialog.executing") : t("gitActionDialog.execute")}
              </Button>
            </>
          ) : (
            <>
              {hasExistingPendingAction ? (
                <Button
                  variant="outline"
                  onClick={handleCancel}
                  disabled={requesting || cancelling}
                >
                  {cancelling
                    ? t("gitActionDialog.cancelling")
                    : t("gitActionDialog.cancelOldGate")}
                </Button>
              ) : null}
              <Button onClick={handleRequest} disabled={requesting || cancelling || !context}>
                {requesting ? t("gitActionDialog.generating") : t("gitActionDialog.generateGate")}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
