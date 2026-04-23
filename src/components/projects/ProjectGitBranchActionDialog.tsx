import { useEffect, useMemo, useState } from "react";

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

const MERGE_FAST_FORWARD_OPTIONS: Array<{ value: GitMergeFastForwardMode; label: string }> = [
  { value: "ff", label: "允许快进（默认）" },
  { value: "no_ff", label: "强制创建合并提交（--no-ff）" },
  { value: "ff_only", label: "仅允许快进（--ff-only）" },
];

const MERGE_STRATEGY_DEFAULT = "__default__";
const MERGE_STRATEGY_OPTIONS: Array<{ value: string; label: string }> = [
  { value: MERGE_STRATEGY_DEFAULT, label: "默认策略" },
  { value: "ort", label: "ort" },
  { value: "recursive", label: "recursive" },
  { value: "resolve", label: "resolve" },
  { value: "ours", label: "ours" },
  { value: "subtree", label: "subtree" },
];

function getDialogTitle(action: ProjectGitBranchActionType | null) {
  switch (action) {
    case "switch":
      return "切换分支";
    case "create":
      return "新建分支";
    case "delete":
      return "删除分支";
    case "merge":
      return "合并分支";
    default:
      return "分支管理";
  }
}

function getDialogDescription(action: ProjectGitBranchActionType | null) {
  switch (action) {
    case "switch":
      return "切换当前项目仓库到指定分支；工作区必须干净，否则将被拒绝。";
    case "create":
      return "基于当前分支或指定分支创建新分支，可选择创建后直接切换。";
    case "delete":
      return "删除指定的本地分支；若分支存在未合并提交，需要勾选强制删除。";
    case "merge":
      return "将源分支合并到目标分支；执行前工作区必须干净。";
    default:
      return "对当前项目仓库执行分支管理操作。";
  }
}

function getSubmitLabel(action: ProjectGitBranchActionType | null, submitting: boolean) {
  if (submitting) {
    switch (action) {
      case "switch":
        return "切换中...";
      case "create":
        return "创建中...";
      case "delete":
        return "删除中...";
      case "merge":
        return "合并中...";
      default:
        return "执行中...";
    }
  }
  switch (action) {
    case "switch":
      return "立即切换";
    case "create":
      return "创建分支";
    case "delete":
      return "立即删除";
    case "merge":
      return "立即合并";
    default:
      return "执行";
  }
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
          setError("请选择要切换的目标分支。");
          return;
        }
        if (switchTarget === currentBranch) {
          setError("目标分支与当前分支相同。");
          return;
        }
      } else if (action === "create") {
        if (!newBranchName.trim()) {
          setError("新分支名不能为空。");
          return;
        }
      } else if (action === "delete") {
        if (!deleteTarget) {
          setError("请选择要删除的分支。");
          return;
        }
        if (deleteTarget === currentBranch) {
          setError("无法删除当前所在分支，请先切换到其他分支。");
          return;
        }
      } else if (action === "merge") {
        if (!mergeSource || !mergeTarget) {
          setError("请选择源分支和目标分支。");
          return;
        }
        if (mergeSource === mergeTarget) {
          setError("源分支和目标分支不能相同。");
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
        <span className="text-xs font-medium text-muted-foreground">目标分支</span>
        <Select<string>
          value={switchTarget}
          onValueChange={(value) => value && setSwitchTarget(value)}
          disabled={submitting || otherBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{switchTarget || "选择分支"}</SelectValue>
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
          当前工作区存在未提交改动，切换会失败。请先提交、回滚或暂存后再切换分支。
        </div>
      )}
    </div>
  );

  const renderCreate = () => (
    <div className="space-y-3">
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">新分支名</span>
        <Input
          value={newBranchName}
          onChange={(event) => setNewBranchName(event.target.value)}
          disabled={submitting}
          placeholder="feature/my-branch"
        />
      </label>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">基准分支</span>
        <Select<string>
          value={createBase}
          onValueChange={(value) => value && setCreateBase(value)}
          disabled={submitting || allBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{createBase || "选择基准分支"}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {allBranches.map((branch) => (
              <SelectItem key={branch} value={branch}>
                {branch}
                {branch === currentBranch ? "（当前）" : ""}
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
        创建后切换到该分支（需工作区干净）
      </label>
      {createAndCheckout && hasWorkingTreeChanges && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
          当前工作区存在未提交改动，勾选"创建后切换"将失败。可取消勾选仅创建分支。
        </div>
      )}
    </div>
  );

  const renderDelete = () => (
    <div className="space-y-3">
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">待删除分支</span>
        <Select<string>
          value={deleteTarget}
          onValueChange={(value) => value && setDeleteTarget(value)}
          disabled={submitting || otherBranches.length === 0}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>{deleteTarget || "选择分支"}</SelectValue>
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
        强制删除（-D，未合并提交将丢失）
      </label>
      {deleteForce && (
        <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          强制删除不会校验分支是否已合并，若分支含有未推送/未合并的独有提交将永久丢失，请确认。
        </div>
      )}
    </div>
  );

  const renderMerge = () => (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="space-y-1.5 block">
          <span className="text-xs font-medium text-muted-foreground">源分支（合并来源）</span>
          <Select<string>
            value={mergeSource}
            onValueChange={(value) => value && setMergeSource(value)}
            disabled={submitting || allBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{mergeSource || "选择源分支"}</SelectValue>
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
          <span className="text-xs font-medium text-muted-foreground">目标分支（合并到）</span>
          <Select<string>
            value={mergeTarget}
            onValueChange={(value) => value && setMergeTarget(value)}
            disabled={submitting || allBranches.length === 0}
          >
            <SelectTrigger className="bg-background">
              <SelectValue>{mergeTarget || "选择目标分支"}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {allBranches
                .filter((branch) => branch !== mergeSource)
                .map((branch) => (
                  <SelectItem key={branch} value={branch}>
                    {branch}
                    {branch === currentBranch ? "（当前）" : ""}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
        </label>
      </div>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">合并方式</span>
        <Select<GitMergeFastForwardMode>
          value={mergeFastForward}
          onValueChange={(value) => value && setMergeFastForward(value)}
          disabled={submitting}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>
              {MERGE_FAST_FORWARD_OPTIONS.find((o) => o.value === mergeFastForward)?.label ?? mergeFastForward}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {MERGE_FAST_FORWARD_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </label>
      <label className="space-y-1.5 block">
        <span className="text-xs font-medium text-muted-foreground">合并策略（可选）</span>
        <Select<string>
          value={mergeStrategy}
          onValueChange={(value) => value && setMergeStrategy(value)}
          disabled={submitting}
        >
          <SelectTrigger className="bg-background">
            <SelectValue>
              {MERGE_STRATEGY_OPTIONS.find((o) => o.value === mergeStrategy)?.label ?? "默认策略"}
            </SelectValue>
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
      {hasWorkingTreeChanges && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
          当前工作区存在未提交改动，合并会被拒绝。请先提交或暂存后再执行。
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
          <div>当前分支：{currentBranch ?? "未知"}</div>
          <div className="mt-1">默认分支：{defaultBranch ?? "未知"}</div>
          <div className="mt-1">本地分支数：{allBranches.length}</div>
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
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={submitting}>
            取消
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
