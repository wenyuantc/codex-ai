import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  checkoutProjectGitBranch,
  createProjectGitBranch,
  deleteProjectGitBranch,
  mergeProjectGitBranches,
} from "@/lib/backend";
import type {
  GitMergeFastForwardMode,
  GitMergeStrategy,
  ProjectGitBranchActionType,
} from "@/lib/types";
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

interface ProjectGitBranchActionDialogProps {
  open: boolean;
  action: ProjectGitBranchActionType | null;
  projectId: string | null;
  currentBranch?: string | null;
  defaultBranch?: string | null;
  projectBranches: string[];
  workingTreeSummary?: string | null;
  onOpenChange: (open: boolean) => void;
  onActionCompleted?: (message: string) => Promise<void> | void;
}

const MERGE_FAST_FORWARD_OPTIONS: Array<{
  value: GitMergeFastForwardMode;
  labelKey: string;
}> = [
  { value: "ff", labelKey: "projects:gitBranchDialog.ff" },
  { value: "no_ff", labelKey: "projects:gitBranchDialog.no_ff" },
  { value: "ff_only", labelKey: "projects:gitBranchDialog.ff_only" },
];

const MERGE_STRATEGY_DEFAULT = "__default__";
const MERGE_STRATEGY_OPTIONS: Array<{ value: string; labelKey: string }> = [
  { value: MERGE_STRATEGY_DEFAULT, labelKey: "projects:gitActionDialog.defaultStrategy" },
  { value: "ort", labelKey: "projects:gitMergeStrategies.ort" },
  { value: "recursive", labelKey: "projects:gitMergeStrategies.recursive" },
  { value: "resolve", labelKey: "projects:gitMergeStrategies.resolve" },
  { value: "ours", labelKey: "projects:gitMergeStrategies.ours" },
  { value: "subtree", labelKey: "projects:gitMergeStrategies.subtree" },
];

function getDialogTitle(action: ProjectGitBranchActionType | null) {
  const key =
    action === "switch" || action === "create" || action === "delete" || action === "merge"
      ? `projects:gitBranchDialog.${action}`
      : "projects:gitBranchDialog.title";
  return i18n.t(key);
}

function getDialogDescription(action: ProjectGitBranchActionType | null) {
  const key =
    action === "switch" || action === "create" || action === "delete" || action === "merge"
      ? `projects:gitBranchDialog.${action}Description`
      : "projects:gitBranchDialog.defaultDescription";
  return i18n.t(key);
}

function getSubmitLabel(action: ProjectGitBranchActionType | null, submitting: boolean) {
  if (submitting) {
    const key =
      action === "switch"
        ? "projects:gitBranchDialog.switching"
        : action === "create"
          ? "projects:creating"
          : action === "delete"
            ? "projects:deleting"
            : action === "merge"
              ? "projects:gitBranchDialog.merging"
              : "projects:gitActionDialog.executing";
    return i18n.t(key);
  }
  const key =
    action === "switch"
      ? "projects:gitBranchDialog.switchNow"
      : action === "create"
        ? "projects:gitBranchDialog.createBranch"
        : action === "delete"
          ? "projects:gitBranchDialog.deleteNow"
          : action === "merge"
            ? "projects:gitBranchDialog.mergeNow"
            : "projects:gitBranchDialog.execute";
  return i18n.t(key);
}

function getFastForwardLabel(forceMode: GitMergeFastForwardMode) {
  const option = MERGE_FAST_FORWARD_OPTIONS.find((item) => item.value === forceMode);
  return option ? i18n.t(option.labelKey) : forceMode;
}

function dedupeBranches(values: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of values) {
    if (!raw) continue;
    const trimmed = raw.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    out.push(trimmed);
  }
  return out;
}

export function ProjectGitBranchActionDialog({
  open,
  action,
  projectId,
  currentBranch,
  defaultBranch,
  projectBranches,
  workingTreeSummary,
  onOpenChange,
  onActionCompleted,
}: ProjectGitBranchActionDialogProps) {
  const { t } = useTranslation("projects");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // switch
  const [switchTarget, setSwitchTarget] = useState("");
  // create
  const [newBranchName, setNewBranchName] = useState("");
  const [createBase, setCreateBase] = useState("");
  const [createAndCheckout, setCreateAndCheckout] = useState(true);
  // delete
  const [deleteTarget, setDeleteTarget] = useState("");
  const [deleteForce, setDeleteForce] = useState(false);
  // merge
  const [mergeSource, setMergeSource] = useState("");
  const [mergeTarget, setMergeTarget] = useState("");
  const [mergeFastForward, setMergeFastForward] = useState<GitMergeFastForwardMode>("ff");
  const [mergeStrategy, setMergeStrategy] = useState<string>(MERGE_STRATEGY_DEFAULT);

  const allBranches = useMemo(
    () => dedupeBranches([currentBranch, defaultBranch, ...projectBranches]),
    [currentBranch, defaultBranch, projectBranches],
  );
  const otherBranches = useMemo(
    () => allBranches.filter((branch) => branch !== currentBranch),
    [allBranches, currentBranch],
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    setError(null);
    setSubmitting(false);
    setSwitchTarget(otherBranches[0] ?? "");
    setNewBranchName("");
    setCreateBase(currentBranch ?? defaultBranch ?? allBranches[0] ?? "");
    setCreateAndCheckout(true);
    setDeleteTarget(otherBranches[0] ?? "");
    setDeleteForce(false);
    const initialTarget = defaultBranch ?? currentBranch ?? allBranches[0] ?? "";
    setMergeTarget(initialTarget);
    setMergeSource(allBranches.find((b) => b !== initialTarget) ?? "");
    setMergeFastForward("ff");
    setMergeStrategy(MERGE_STRATEGY_DEFAULT);
  }, [action, open, currentBranch, defaultBranch, allBranches, otherBranches]);

  const hasWorkingTreeChanges = Boolean(workingTreeSummary);

  const shouldRenderDialog = open && Boolean(projectId) && action !== null;
  if (!shouldRenderDialog) {
    return null;
  }

  const handleSubmit = async () => {
    if (!projectId || !action) return;
    setError(null);

    try {
      if (action === "switch") {
        if (!switchTarget) {
          setError(t("gitBranchDialog.selectSwitchError"));
          return;
        }
        if (switchTarget === currentBranch) {
          setError(t("gitBranchDialog.sameBranchError"));
          return;
        }
      } else if (action === "create") {
        if (!newBranchName.trim()) {
          setError(t("gitBranchDialog.emptyBranchNameError"));
          return;
        }
      } else if (action === "delete") {
        if (!deleteTarget) {
          setError(t("gitBranchDialog.selectDeleteError"));
          return;
        }
        if (deleteTarget === currentBranch) {
          setError(t("gitBranchDialog.deleteCurrentError"));
          return;
        }
      } else if (action === "merge") {
        if (!mergeSource || !mergeTarget) {
          setError(t("gitBranchDialog.selectMergeError"));
          return;
        }
        if (mergeSource === mergeTarget) {
          setError(t("gitBranchDialog.sameMergeError"));
          return;
        }
      }

      setSubmitting(true);
      let message: string;
      if (action === "switch") {
        message = await checkoutProjectGitBranch(projectId, switchTarget);
      } else if (action === "create") {
        message = await createProjectGitBranch(
          projectId,
          newBranchName.trim(),
          createBase || null,
          createAndCheckout,
        );
      } else if (action === "delete") {
        message = await deleteProjectGitBranch(projectId, deleteTarget, deleteForce);
      } else {
        const strategyArg =
          mergeStrategy === MERGE_STRATEGY_DEFAULT ? null : (mergeStrategy as GitMergeStrategy);
        message = await mergeProjectGitBranches(
          projectId,
          mergeSource,
          mergeTarget,
          mergeFastForward,
          strategyArg,
        );
      }
      await onActionCompleted?.(message);
      onOpenChange(false);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    } finally {
      setSubmitting(false);
    }
  };

  const renderSwitch = () => (
    <div className="space-y-3">
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitActionDialog.targetBranchField")}
        </span>
        <Select<string>
          value={switchTarget}
          onValueChange={(value) => value && setSwitchTarget(value)}
          disabled={submitting || otherBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{switchTarget || t("gitBranchDialog.selectBranch")}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {otherBranches.map((branch) => (
              <SelectItem key={branch} value={branch}>
                {branch}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </label>
      {hasWorkingTreeChanges && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
          {t("gitBranchDialog.switchDirtyHint")}
        </div>
      )}
    </div>
  );

  const renderCreate = () => (
    <div className="space-y-3">
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitBranchDialog.newBranchName")}
        </span>
        <Input
          value={newBranchName}
          onChange={(event) => setNewBranchName(event.target.value)}
          disabled={submitting}
          placeholder="feature/my-branch"
        />
      </label>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitBranchDialog.baseBranch")}
        </span>
        <Select<string>
          value={createBase}
          onValueChange={(value) => value && setCreateBase(value)}
          disabled={submitting || allBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{createBase || t("gitBranchDialog.selectBaseBranch")}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {allBranches.map((branch) => (
              <SelectItem key={branch} value={branch}>
                {branch}
                {branch === currentBranch ? t("gitBranchDialog.currentSuffix") : ""}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </label>
      <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
        <input
          type="checkbox"
          checked={createAndCheckout}
          onChange={(event) => setCreateAndCheckout(event.target.checked)}
          disabled={submitting}
        />
        {t("gitBranchDialog.checkoutAfterCreate")}
      </label>
      {createAndCheckout && hasWorkingTreeChanges && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
          {t("gitBranchDialog.createDirtyHint")}
        </div>
      )}
    </div>
  );

  const renderDelete = () => (
    <div className="space-y-3">
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitBranchDialog.deleteTarget")}
        </span>
        <Select<string>
          value={deleteTarget}
          onValueChange={(value) => value && setDeleteTarget(value)}
          disabled={submitting || otherBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{deleteTarget || t("gitBranchDialog.selectBranch")}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {otherBranches.map((branch) => (
              <SelectItem key={branch} value={branch}>
                {branch}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </label>
      <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
        <input
          type="checkbox"
          checked={deleteForce}
          onChange={(event) => setDeleteForce(event.target.checked)}
          disabled={submitting}
        />
        {t("gitBranchDialog.forceDelete")}
      </label>
      {deleteForce && (
        <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {t("gitBranchDialog.forceDeleteHint")}
        </div>
      )}
    </div>
  );

  const renderMerge = () => (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="space-y-1.5 block">
          <span className="text-xs font-medium text-muted-foreground">
            {t("gitBranchDialog.mergeSource")}
          </span>
          <Select<string>
            value={mergeSource}
            onValueChange={(value) => value && setMergeSource(value)}
            disabled={submitting || allBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{mergeSource || t("gitBranchDialog.selectSource")}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {allBranches
                .filter((branch) => branch !== mergeTarget)
                .map((branch) => (
                  <SelectItem key={branch} value={branch}>
                    {branch}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
        </label>
        <label className="space-y-1.5 block">
          <span className="text-xs font-medium text-muted-foreground">
            {t("gitBranchDialog.mergeTargetField")}
          </span>
          <Select<string>
            value={mergeTarget}
            onValueChange={(value) => value && setMergeTarget(value)}
            disabled={submitting || allBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{mergeTarget || t("gitActionDialog.selectTargetBranch")}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {allBranches
                .filter((branch) => branch !== mergeSource)
                .map((branch) => (
                  <SelectItem key={branch} value={branch}>
                    {branch}
                    {branch === currentBranch ? t("gitBranchDialog.currentSuffix") : ""}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
        </label>
      </div>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitBranchDialog.mergeMode")}
        </span>
        <Select<GitMergeFastForwardMode>
          value={mergeFastForward}
          onValueChange={(value) => value && setMergeFastForward(value)}
          disabled={submitting}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{getFastForwardLabel(mergeFastForward)}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {MERGE_FAST_FORWARD_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {i18n.t(option.labelKey)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </label>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">
          {t("gitBranchDialog.mergeStrategyOptional")}
        </span>
        <Select<string>
          value={mergeStrategy}
          onValueChange={(value) => value && setMergeStrategy(value)}
          disabled={submitting}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>
              {MERGE_STRATEGY_OPTIONS.find((o) => o.value === mergeStrategy)?.labelKey
                ? i18n.t(MERGE_STRATEGY_OPTIONS.find((o) => o.value === mergeStrategy)!.labelKey)
                : i18n.t("projects:gitActionDialog.defaultStrategy")}
            </SelectValue>
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
      {hasWorkingTreeChanges && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
          {t("gitBranchDialog.mergeDirtyHint")}
        </div>
      )}
    </div>
  );

  const primaryDisabled =
    submitting ||
    (action === "switch" && (!switchTarget || otherBranches.length === 0)) ||
    (action === "create" && !newBranchName.trim()) ||
    (action === "delete" && (!deleteTarget || otherBranches.length === 0)) ||
    (action === "merge" && (!mergeSource || !mergeTarget || mergeSource === mergeTarget));

  const primaryVariant = action === "delete" && deleteForce ? "destructive" : undefined;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{getDialogTitle(action)}</DialogTitle>
          <DialogDescription>{getDialogDescription(action)}</DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>
            {t("gitBranchDialog.currentBranchSummary", {
              branch: currentBranch ?? t("common:unknown"),
            })}
          </div>
          <div className="mt-1">
            {t("gitBranchDialog.defaultBranchSummary", {
              branch: defaultBranch ?? t("common:unknown"),
            })}
          </div>
          <div className="mt-1">
            {t("gitBranchDialog.branchCountSummary", { count: allBranches.length })}
          </div>
        </div>

        {action === "switch" && renderSwitch()}
        {action === "create" && renderCreate()}
        {action === "delete" && renderDelete()}
        {action === "merge" && renderMerge()}

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
            variant={primaryVariant}
            onClick={() => void handleSubmit()}
            disabled={primaryDisabled}
          >
            {getSubmitLabel(action, submitting)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
