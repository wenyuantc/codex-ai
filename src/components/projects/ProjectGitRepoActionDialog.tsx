import { useEffect, useState } from "react";

import {
  commitProjectGitChanges,
  pullProjectGitBranch,
  pushProjectGitBranch,
} from "@/lib/backend";
import { aiGenerateCommitMessage } from "@/lib/codex";
import type { ProjectGitRepoActionType, ProjectGitWorkingTreeChange } from "@/lib/types";
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
import { Loader2, Sparkles } from "lucide-react";

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

const PUSH_FORCE_MODE_OPTIONS: Array<{ value: PushForceMode; label: string }> = [
  { value: "none", label: "普通推送" },
  { value: "force", label: "强制推送（force）" },
  { value: "force_with_lease", label: "保护式强推（force-with-lease）" },
];

const PULL_MODE_OPTIONS: Array<{ value: PullMode; label: string }> = [
  { value: "ff_only", label: "仅允许快进（ff-only）" },
  { value: "rebase", label: "变基拉取（rebase）" },
];

function getDialogTitle(action: ProjectGitRepoActionType | null) {
  switch (action) {
    case "commit":
      return "提交已暂存改动";
    case "push":
      return "推送当前分支";
    case "pull":
      return "拉取远端更新";
    default:
      return "仓库操作";
  }
}

function getDialogDescription(action: ProjectGitRepoActionType | null, stagedFileCount: number) {
  switch (action) {
    case "commit":
      return stagedFileCount > 0
        ? `当前已有 ${stagedFileCount} 个文件处于已暂存状态，将基于这些改动创建提交。`
        : "当前没有已暂存文件，请先在工作区文件列表中暂存改动后再提交。";
    case "push":
      return "将当前分支推送到指定远端。首次推送新分支时，也可直接指定分支名。";
    case "pull":
      return "从指定远端拉取当前分支更新，可选择快进拉取或 rebase 拉取。";
    default:
      return "对当前项目仓库执行提交、推送或拉取操作。";
  }
}

function getSubmitLabel(action: ProjectGitRepoActionType | null, submitting: boolean) {
  if (submitting) {
    switch (action) {
      case "commit":
        return "提交中...";
      case "push":
        return "推送中...";
      case "pull":
        return "拉取中...";
      default:
        return "执行中...";
    }
  }

  switch (action) {
    case "commit":
      return "创建提交";
    case "push":
      return "立即推送";
    case "pull":
      return "立即拉取";
    default:
      return "执行";
  }
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
    setPullMode(Boolean(workingTreeSummary) ? "rebase" : "ff_only");
    setPullAutoStash(Boolean(workingTreeSummary));
    setError(null);
  }, [action, currentBranch, open, projectBranches, workingTreeSummary]);

  const branchOptions = buildBranchOptions(currentBranch, projectBranches, branchName);

  const stagedChangePrompts = stagedChanges.map((change) => {
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

  const handleGenerateCommitMessage = async () => {
    if (!projectId) {
      setError("当前项目信息不完整，无法生成提交信息。");
      return;
    }
    if (stagedChangePrompts.length === 0) {
      setError("当前没有已暂存文件，无法生成提交信息。");
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
        setError("当前没有已暂存文件，请先暂存后再提交。");
        return;
      }
      if (!commitMessage.trim()) {
        setError("提交说明不能为空。");
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
            setError("提交已创建，但当前分支未知，请返回后手动推送。");
            return;
          }
          try {
            const pushResult = await pushProjectGitBranch(projectId, "origin", branch, "none");
            result = `${commitResult}\n${pushResult}`;
          } catch (pushError) {
            await onActionCompleted?.(commitResult);
            setError(
              `提交已创建，但推送失败：${pushError instanceof Error ? pushError.message : String(pushError)}`,
            );
            return;
          }
        } else {
          result = commitResult;
        }
      } else if (action === "push") {
        result = await pushProjectGitBranch(projectId, remoteName.trim(), branchName.trim(), pushForceMode);
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{getDialogTitle(action)}</DialogTitle>
          <DialogDescription>{getDialogDescription(action, stagedFileCount)}</DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>当前分支：{currentBranch ?? "未知"}</div>
          <div className="mt-1">已暂存文件：{stagedFileCount}</div>
        </div>

        {action === "commit" ? (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium text-muted-foreground">提交说明</span>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                onClick={() => void handleGenerateCommitMessage()}
                disabled={submitting || generatingCommitMessage || stagedFileCount === 0}
                title="AI 生成提交信息"
              >
                {generatingCommitMessage ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Sparkles className="h-4 w-4" />
                )}
              </Button>
            </div>
            <Textarea
              value={commitMessage}
              onChange={(event) => setCommitMessage(event.target.value)}
              disabled={submitting || generatingCommitMessage}
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
        ) : action === "push" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">远端名称</span>
              <Input
                value={remoteName}
                onChange={(event) => setRemoteName(event.target.value)}
                disabled={submitting}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">分支名称</span>
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
                  <SelectValue>{branchName || "选择分支"}</SelectValue>
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
              <span className="text-xs font-medium text-muted-foreground">推送模式</span>
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
                  <SelectValue>
                    {PUSH_FORCE_MODE_OPTIONS.find((option) => option.value === pushForceMode)?.label ?? pushForceMode}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {PUSH_FORCE_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
          </div>
        ) : action === "pull" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">远端名称</span>
              <Input
                value={remoteName}
                onChange={(event) => setRemoteName(event.target.value)}
                disabled={submitting}
                placeholder="origin"
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">分支名称</span>
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
                  <SelectValue>{branchName || "选择分支"}</SelectValue>
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
              <span className="text-xs font-medium text-muted-foreground">拉取方式</span>
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
                  <SelectValue>
                    {PULL_MODE_OPTIONS.find((option) => option.value === pullMode)?.label ?? pullMode}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {PULL_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
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
              拉取前自动暂存本地改动（autostash）
            </label>
            {hasWorkingTreeChanges && (
              <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900 sm:col-span-2">
                {pullMode === "rebase"
                  ? "当前工作区存在本地改动，已默认切到 rebase 拉取。建议保留 autostash，避免未提交改动导致拉取失败。"
                  : "当前工作区存在本地改动。若仅需快进，可保留 autostash；如果再次出现“Not possible to fast-forward”，说明本地分支与远端已分叉，请改用 rebase 拉取。"}
              </div>
            )}
          </div>
        ) : null}

        {branchOptions.length === 0 && action !== "commit" && (
          <div className="rounded-md border border-dashed border-border px-3 py-2 text-xs text-muted-foreground">
            当前没有可选分支，请先确认仓库处于正常分支状态后再执行此操作。
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
            取消
          </Button>
          {action === "commit" && (
            <Button
              type="button"
              variant="outline"
              onClick={() => void handleSubmit("commit_push")}
              disabled={submitting || commitDisabled || !currentBranch}
              title={currentBranch ? `提交后推送到 origin/${currentBranch}` : "当前分支未知，暂不可提交并推送"}
            >
              {submitMode === "commit_push" ? "提交并推送中..." : "提交并推送"}
            </Button>
          )}
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
