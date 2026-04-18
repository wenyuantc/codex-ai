import { useEffect, useState } from "react";

import {
  cancelGitAction,
  confirmGitAction,
  requestGitAction,
} from "@/lib/backend";
import type { GitActionType, TaskGitContext } from "@/lib/types";
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

const GIT_ACTION_OPTIONS: Array<{ value: GitActionType; label: string }> = [
  { value: "merge", label: "将任务分支合并到目标分支" },
  { value: "push", label: "推送分支" },
  { value: "rebase", label: "变基到目标分支" },
  { value: "cherry_pick", label: "挑拣提交（Cherry-pick）" },
  { value: "stash", label: "暂存当前改动（Stash）" },
  { value: "unstash", label: "恢复暂存改动（Unstash）" },
  { value: "cleanup_worktree", label: "清理任务工作树" },
];

const FORCE_MODE_OPTIONS: Array<{
  value: GitActionFormState["forceMode"];
  label: string;
}> = [
  { value: "none", label: "普通推送" },
  { value: "force", label: "强制推送（force）" },
  { value: "force_with_lease", label: "带保护强推（force-with-lease）" },
];

const MERGE_STRATEGY_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "ort", label: "默认策略（ort）" },
  { value: "recursive", label: "递归策略（recursive）" },
  { value: "resolve", label: "快速冲突解决（resolve）" },
  { value: "ours", label: "优先保留当前分支（ours）" },
  { value: "subtree", label: "子树合并（subtree）" },
];

function getTaskGitContextStateLabel(state: string) {
  switch (state) {
    case "provisioning":
      return "准备中";
    case "ready":
      return "可执行";
    case "running":
      return "执行中";
    case "merge_ready":
      return "待合并";
    case "action_pending":
      return "待确认";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "drifted":
      return "上下文失效";
    default:
      return state;
  }
}

function getGitActionLabel(actionType: GitActionType) {
  return GIT_ACTION_OPTIONS.find((option) => option.value === actionType)?.label ?? actionType;
}

function getMergeStrategyLabel(strategy: string) {
  return MERGE_STRATEGY_OPTIONS.find((option) => option.value === strategy)?.label ?? strategy;
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
    strategy:
      typeof payload?.strategy === "string" ? payload.strategy : "ort",
    allowFastForward:
      typeof payload?.allow_ff === "boolean" ? payload.allow_ff : true,
    remoteName:
      typeof payload?.remote_name === "string" ? payload.remote_name : "origin",
    sourceBranch:
      typeof payload?.source_branch === "string"
        ? payload.source_branch
        : (context?.task_branch ?? ""),
    targetRef:
      typeof payload?.target_ref === "string"
        ? payload.target_ref
        : (context?.task_branch ?? ""),
    forceMode:
      payload?.force_mode === "force" || payload?.force_mode === "force_with_lease"
        ? payload.force_mode
        : "none",
    ontoBranch:
      typeof payload?.onto_branch === "string"
        ? payload.onto_branch
        : (context?.target_branch ?? ""),
    autoStash:
      typeof payload?.auto_stash === "boolean" ? payload.auto_stash : false,
    cherryPickCommitIds: Array.isArray(payload?.commit_ids)
      ? payload.commit_ids.filter((item): item is string => typeof item === "string").join("\n")
      : "",
    includeUntracked:
      typeof payload?.include_untracked === "boolean" ? payload.include_untracked : false,
    stashMessage:
      typeof payload?.message === "string" ? payload.message : "",
    stashRef:
      typeof payload?.stash_ref === "string" ? payload.stash_ref : "stash@{0}",
    deleteBranch:
      typeof payload?.delete_branch === "boolean" ? payload.delete_branch : false,
    pruneWorktree:
      typeof payload?.prune_worktree === "boolean" ? payload.prune_worktree : true,
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
      preferredAction
      ?? context?.pending_action_type
      ?? (context?.state === "drifted" ? "cleanup_worktree" : "merge");
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
        `任务分支：${context.task_branch ?? "未命名分支"}`,
        `目标分支：${context.target_branch ?? "未设置"}`,
        `当前状态：${getTaskGitContextStateLabel(context.state)}`,
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
      setInfo(`已生成“${getGitActionLabel(selectedAction)}”确认门，请确认执行或取消。`);
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
          ? `已取消“${getGitActionLabel(selectedAction)}”确认门。`
          : "已取消当前待确认的 Git 动作。",
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
              <span className="text-xs font-medium text-muted-foreground">目标分支</span>
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
                  <SelectValue>{form.targetBranch || "选择目标分支"}</SelectValue>
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
              <span className="text-xs font-medium text-muted-foreground">合并策略</span>
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
                      {option.label}
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
              允许 fast-forward 合并
            </label>
          </div>
        );
      case "push":
        return (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">远端名称</span>
              <Input
                value={form.remoteName}
                onChange={(event) => updateForm({ remoteName: event.target.value })}
                disabled={formLocked}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">源分支</span>
              <Input
                value={form.sourceBranch}
                onChange={(event) => updateForm({ sourceBranch: event.target.value })}
                disabled={formLocked}
                placeholder="codex/task-..."
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">目标引用</span>
              <Input
                value={form.targetRef}
                onChange={(event) => updateForm({ targetRef: event.target.value })}
                disabled={formLocked}
                placeholder="codex/task-..."
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">推送模式</span>
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
                  <SelectValue>
                    {FORCE_MODE_OPTIONS.find((option) => option.value === form.forceMode)?.label ?? form.forceMode}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {FORCE_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
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
              <span className="text-xs font-medium text-muted-foreground">目标分支</span>
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
                  <SelectValue>{form.ontoBranch || "选择目标分支"}</SelectValue>
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
              Rebase 时启用 auto-stash
            </label>
          </div>
        );
      case "cherry_pick":
        return (
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">提交 SHA</span>
            <Textarea
              value={form.cherryPickCommitIds}
              onChange={(event) => updateForm({ cherryPickCommitIds: event.target.value })}
              disabled={formLocked}
              placeholder={"每行一个 commit SHA\n例如：\nabc1234\ndef5678"}
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
              包含未跟踪文件
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">Stash 备注</span>
              <Input
                value={form.stashMessage}
                onChange={(event) => updateForm({ stashMessage: event.target.value })}
                disabled={formLocked}
                placeholder="可选"
              />
            </label>
          </div>
        );
      case "unstash":
        return (
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">Stash 引用</span>
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
              清理时同时删除任务分支
            </label>
            <label className="flex items-center gap-2 rounded-md border border-border/60 px-3 py-2 text-sm">
              <input
                type="checkbox"
                checked={form.pruneWorktree}
                onChange={(event) => updateForm({ pruneWorktree: event.target.checked })}
                disabled={formLocked}
              />
              清理后执行 worktree prune
            </label>
          </div>
        );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,42rem)] max-w-[min(96vw,42rem)] sm:max-w-[min(96vw,42rem)]">
        <DialogHeader>
          <DialogTitle>Git 高风险动作确认门</DialogTitle>
          <DialogDescription>
            先 request 生成一次性 token，再决定 confirm 或 cancel。重新 request 会自动使旧 token 失效。
          </DialogDescription>
        </DialogHeader>

        {context ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-border/60 bg-secondary/20 px-3 py-2 text-xs text-muted-foreground">
              {actionSummary}
            </div>

            {hasExistingPendingAction && pendingToken === null && (
              <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
                当前上下文已有“{getGitActionLabel(context.pending_action_type ?? "merge")}”待确认动作。
                由于明文 token 不会回传到前端，你可以直接取消旧确认门，或重新 request 一次新动作。
              </div>
            )}

            <div className="space-y-3">
              <label className="space-y-1.5">
                <span className="text-xs font-medium text-muted-foreground">动作类型</span>
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
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
              {lockActionSelection && preferredAction ? (
                <div className="rounded-lg border border-border/60 bg-secondary/20 px-3 py-2 text-xs text-muted-foreground">
                  当前入口已锁定为“{getGitActionLabel(preferredAction)}”，如需其他 Git 动作，请从项目详情页进入通用 Git 动作面板。
                </div>
              ) : null}

              {renderActionFields()}
            </div>

            {pendingToken ? (
              <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">确认门已生成</div>
                <div className="mt-1 break-all">token：{pendingToken}</div>
                <div className="mt-1">过期时间：{pendingExpiresAt ?? "未知"}</div>
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
                重新选择
              </Button>
              <Button
                variant="outline"
                onClick={handleCancel}
                disabled={confirming || cancelling}
              >
                {cancelling ? "取消中..." : "取消确认门"}
              </Button>
              <Button onClick={handleConfirm} disabled={confirming || cancelling}>
                {confirming ? "执行中..." : "确认执行"}
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
                  {cancelling ? "取消中..." : "取消旧确认门"}
                </Button>
              ) : null}
              <Button onClick={handleRequest} disabled={requesting || cancelling || !context}>
                {requesting ? "生成中..." : "生成确认门"}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
