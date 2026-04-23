import { Suspense, lazy, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  Copy,
  Loader2,
  RefreshCw,
  Trash2,
} from "lucide-react";

import {
  getProjectWorktreeFilePreview,
  listProjectGitWorktrees,
  removeProjectGitWorktree,
  rollbackAllProjectWorktreeChanges,
  rollbackProjectWorktreeFiles,
  stageAllProjectWorktreeFiles,
  stageProjectWorktreeFile,
  unstageAllProjectWorktreeFiles,
  unstageProjectWorktreeFile,
} from "@/lib/backend";
import { countStagedGitFiles } from "@/lib/gitWorkingTree";
import type {
  ProjectGitFilePreview,
  ProjectGitWorktree,
  ProjectGitWorkingTreeChange,
} from "@/lib/types";
import { GitChangesPanel } from "@/components/git/GitChangesPanel";
import { WorktreeCommitDialog } from "@/components/git/WorktreeCommitDialog";
import { WorktreeMergeDialog } from "@/components/git/WorktreeMergeDialog";
import { getGitActionButtonClassName } from "@/components/git/gitHelpers";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ProjectWorktreeSectionProps {
  projectId: string;
  currentBranch?: string | null;
  defaultBranch?: string | null;
  projectBranches: string[];
  onChanged?: () => Promise<void> | void;
}

const ProjectGitFilePreviewDialog = lazy(async () => {
  const module = await import("@/components/projects/ProjectGitFilePreviewDialog");
  return { default: module.ProjectGitFilePreviewDialog };
});

function normalizeNextStageStatus(change: ProjectGitWorkingTreeChange, action: "stage" | "unstage") {
  if (action === "stage") {
    return "staged" as const;
  }
  return change.change_type === "added" && !change.previous_path ? "untracked" as const : "unstaged" as const;
}

export function ProjectWorktreeSection({
  projectId,
  currentBranch,
  defaultBranch,
  projectBranches,
  onChanged,
}: ProjectWorktreeSectionProps) {
  const [open, setOpen] = useState(false);
  const [worktrees, setWorktrees] = useState<ProjectGitWorktree[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ tone: "success" | "error"; message: string } | null>(null);
  const [expandedPath, setExpandedPath] = useState<string | null>(null);
  const [stagingFileState, setStagingFileState] = useState<{ worktreePath: string; filePath: string } | null>(null);
  const [bulkStageState, setBulkStageState] = useState<{ worktreePath: string; action: "stage_all" | "unstage_all" } | null>(null);
  const [selectedFilesStageState, setSelectedFilesStageState] = useState<{ worktreePath: string; action: "stage" | "unstage" } | null>(null);
  const [rollbackConfirm, setRollbackConfirm] = useState<{
    worktreePath: string;
    target: "selected" | "all";
    paths: string[];
  } | null>(null);
  const [rollbackInProgressPath, setRollbackInProgressPath] = useState<string | null>(null);
  const [selectedPreview, setSelectedPreview] = useState<{
    worktreePath: string;
    change: ProjectGitWorkingTreeChange;
  } | null>(null);
  const [preview, setPreview] = useState<ProjectGitFilePreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [selectedCommitPath, setSelectedCommitPath] = useState<string | null>(null);
  const [selectedMergePath, setSelectedMergePath] = useState<string | null>(null);
  const [pendingRemove, setPendingRemove] = useState<ProjectGitWorktree | null>(null);
  const [removeMode, setRemoveMode] = useState<"normal" | "force" | null>(null);
  const [copiedPath, setCopiedPath] = useState<string | null>(null);
  const previewRequestIdRef = useRef(0);
  const copyResetTimerRef = useRef<number | null>(null);

  const selectedCommitWorktree = useMemo(
    () => worktrees.find((worktree) => worktree.path === selectedCommitPath) ?? null,
    [selectedCommitPath, worktrees],
  );
  const selectedMergeWorktree = useMemo(
    () => worktrees.find((worktree) => worktree.path === selectedMergePath) ?? null,
    [selectedMergePath, worktrees],
  );

  const loadWorktrees = async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await listProjectGitWorktrees(projectId);
      setWorktrees(items);
      setExpandedPath((current) => items.some((item) => item.path === current) ? current : null);
      setSelectedCommitPath((current) => items.some((item) => item.path === current) ? current : null);
      setSelectedMergePath((current) => items.some((item) => item.path === current) ? current : null);
      setPendingRemove((current) => current && items.some((item) => item.path === current.path) ? current : null);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadWorktrees();
  }, [projectId]);

  useEffect(() => () => {
    if (copyResetTimerRef.current !== null) {
      window.clearTimeout(copyResetTimerRef.current);
    }
  }, []);

  const updateWorktreeChanges = (
    worktreePath: string,
    updater: (change: ProjectGitWorkingTreeChange) => ProjectGitWorkingTreeChange,
  ) => {
    setWorktrees((current) => current.map((worktree) => {
      if (worktree.path !== worktreePath) {
        return worktree;
      }
      return {
        ...worktree,
        working_tree_changes: worktree.working_tree_changes.map(updater),
      };
    }));
    setSelectedPreview((current) => {
      if (!current || current.worktreePath !== worktreePath) {
        return current;
      }
      return {
        ...current,
        change: updater(current.change),
      };
    });
  };

  const handleCopyPath = async (path: string) => {
    if (typeof navigator === "undefined" || typeof navigator.clipboard?.writeText !== "function") {
      return;
    }

    await navigator.clipboard.writeText(path);
    setCopiedPath(path);
    if (copyResetTimerRef.current !== null) {
      window.clearTimeout(copyResetTimerRef.current);
    }
    copyResetTimerRef.current = window.setTimeout(() => {
      setCopiedPath(null);
    }, 1600);
  };

  const handlePreview = async (worktree: ProjectGitWorktree, change: ProjectGitWorkingTreeChange) => {
    const requestId = previewRequestIdRef.current + 1;
    previewRequestIdRef.current = requestId;
    setSelectedPreview({ worktreePath: worktree.path, change });
    setPreview(null);
    setPreviewError(null);
    setPreviewLoading(true);
    try {
      const nextPreview = await getProjectWorktreeFilePreview(
        projectId,
        worktree.path,
        change.path,
        change.previous_path,
        change.change_type,
      );
      if (previewRequestIdRef.current !== requestId) {
        return;
      }
      setPreview(nextPreview);
    } catch (previewLoadError) {
      if (previewRequestIdRef.current !== requestId) {
        return;
      }
      setPreviewError(previewLoadError instanceof Error ? previewLoadError.message : String(previewLoadError));
    } finally {
      if (previewRequestIdRef.current === requestId) {
        setPreviewLoading(false);
      }
    }
  };

  const handleToggleStage = async (worktree: ProjectGitWorktree, change: ProjectGitWorkingTreeChange) => {
    setStagingFileState({ worktreePath: worktree.path, filePath: change.path });
    setNotice(null);
    try {
      const action = change.stage_status === "staged" || change.stage_status === "partially_staged"
        ? "unstage"
        : "stage";
      const message = action === "stage"
        ? await stageProjectWorktreeFile(projectId, worktree.path, change.path)
        : await unstageProjectWorktreeFile(projectId, worktree.path, change.path);
      updateWorktreeChanges(worktree.path, (item) => item.path === change.path
        ? { ...item, stage_status: normalizeNextStageStatus(item, action) }
        : item);
      setNotice({ tone: "success", message });
    } catch (stageError) {
      setNotice({ tone: "error", message: stageError instanceof Error ? stageError.message : String(stageError) });
    } finally {
      setStagingFileState(null);
    }
  };

  const handleBulkStage = async (
    worktree: ProjectGitWorktree,
    action: "stage_all" | "unstage_all",
  ) => {
    setBulkStageState({ worktreePath: worktree.path, action });
    setNotice(null);
    try {
      const message = action === "stage_all"
        ? await stageAllProjectWorktreeFiles(projectId, worktree.path)
        : await unstageAllProjectWorktreeFiles(projectId, worktree.path);
      updateWorktreeChanges(worktree.path, (change) => ({
        ...change,
        stage_status: action === "stage_all"
          ? "staged"
          : normalizeNextStageStatus(change, "unstage"),
      }));
      setNotice({ tone: "success", message });
    } catch (bulkError) {
      setNotice({ tone: "error", message: bulkError instanceof Error ? bulkError.message : String(bulkError) });
    } finally {
      setBulkStageState(null);
    }
  };

  const handleStageSelected = async (
    worktree: ProjectGitWorktree,
    action: "stage" | "unstage",
    paths: string[],
  ) => {
    if (paths.length === 0) {
      return;
    }
    setSelectedFilesStageState({ worktreePath: worktree.path, action });
    setNotice(null);
    try {
      for (const path of paths) {
        if (action === "stage") {
          await stageProjectWorktreeFile(projectId, worktree.path, path);
        } else {
          await unstageProjectWorktreeFile(projectId, worktree.path, path);
        }
      }
      updateWorktreeChanges(worktree.path, (change) => (
        paths.includes(change.path)
          ? { ...change, stage_status: normalizeNextStageStatus(change, action) }
          : change
      ));
      setNotice({
        tone: "success",
        message: `已${action === "stage" ? "暂存" : "取消暂存"} ${paths.length} 个文件`,
      });
    } catch (selectedError) {
      setNotice({ tone: "error", message: selectedError instanceof Error ? selectedError.message : String(selectedError) });
    } finally {
      setSelectedFilesStageState(null);
    }
  };

  const handleRollback = (worktree: ProjectGitWorktree, target: "selected" | "all", paths: string[] = []) => {
    setRollbackConfirm({
      worktreePath: worktree.path,
      target,
      paths,
    });
  };

  const confirmRollback = async () => {
    if (!rollbackConfirm) {
      return;
    }
    setRollbackInProgressPath(rollbackConfirm.worktreePath);
    setNotice(null);
    try {
      const message = rollbackConfirm.target === "all"
        ? await rollbackAllProjectWorktreeChanges(projectId, rollbackConfirm.worktreePath)
        : await rollbackProjectWorktreeFiles(projectId, rollbackConfirm.worktreePath, rollbackConfirm.paths);
      setNotice({ tone: "success", message });
      setRollbackConfirm(null);
      await loadWorktrees();
    } catch (rollbackError) {
      setNotice({ tone: "error", message: rollbackError instanceof Error ? rollbackError.message : String(rollbackError) });
    } finally {
      setRollbackInProgressPath(null);
    }
  };

  const handleRemoveClick = (worktree: ProjectGitWorktree) => {
    if (worktree.is_main) {
      return;
    }
    if (worktree.is_locked) {
      setNotice({
        tone: "error",
        message: worktree.lock_reason
          ? `当前 worktree 已锁定，请先解锁：${worktree.lock_reason}`
          : "当前 worktree 已锁定，请先解锁后再删除。",
      });
      return;
    }
    setPendingRemove(worktree);
  };

  const confirmRemove = async (mode: "normal" | "force") => {
    if (!pendingRemove) {
      return;
    }
    setRemoveMode(mode);
    setNotice(null);
    try {
      const message = await removeProjectGitWorktree(projectId, pendingRemove.path, mode === "force");
      setNotice({ tone: "success", message });
      setPendingRemove(null);
      if (expandedPath === pendingRemove.path) {
        setExpandedPath(null);
      }
      await loadWorktrees();
      await onChanged?.();
    } catch (removeError) {
      setNotice({ tone: "error", message: removeError instanceof Error ? removeError.message : String(removeError) });
    } finally {
      setRemoveMode(null);
    }
  };

  return (
    <>
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="rounded-lg border border-border/60">
          <CollapsibleTrigger className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left">
            <div>
              <h4 className="text-sm font-medium">Worktree 管理（{worktrees.length}）</h4>
              <p className="text-[11px] text-muted-foreground">
                统一查看当前项目的 Git worktree，支持预览、暂存、回滚、提交、合并与删除。
              </p>
            </div>
            <ChevronDown className={`h-4 w-4 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`} />
          </CollapsibleTrigger>

          <CollapsibleContent className="border-t border-border/60 px-4 py-3">
            <div className="space-y-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="text-xs text-muted-foreground">
                  {loading ? "正在刷新 worktree 列表..." : `共发现 ${worktrees.length} 个 worktree`}
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={loading}
                  onClick={() => void loadWorktrees()}
                >
                  {loading ? (
                    <>
                      <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />
                      刷新中...
                    </>
                  ) : (
                    <>
                      <RefreshCw className="mr-2 h-3.5 w-3.5" />
                      刷新
                    </>
                  )}
                </Button>
              </div>

              {notice && (
                <div
                  className={`rounded-md border px-3 py-2 text-xs ${
                    notice.tone === "success"
                      ? "border-primary/20 bg-primary/5 text-primary"
                      : "border-destructive/20 bg-destructive/10 text-destructive"
                  }`}
                >
                  {notice.message}
                </div>
              )}

              {error && (
                <div className="rounded-md border border-destructive/20 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                  {error}
                </div>
              )}

              {!loading && worktrees.length === 0 && !error && (
                <div className="rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
                  当前项目没有可展示的 Git worktree。
                </div>
              )}

              {worktrees.map((worktree) => {
                const hasChanges = worktree.working_tree_changes.length > 0;
                const hasStagedChanges = countStagedGitFiles(worktree.working_tree_changes) > 0;
                const isExpanded = expandedPath === worktree.path;
                const removeDisabled = worktree.is_main;
                const mergeDisabled =
                  worktree.is_main
                  || worktree.is_bare
                  || worktree.is_prunable
                  || worktree.is_detached
                  || !worktree.branch;
                const statusLabel = worktree.branch ?? (worktree.is_detached
                  ? `detached HEAD${worktree.short_head_sha ? ` · ${worktree.short_head_sha}` : ""}`
                  : "未知分支");

                return (
                  <div key={worktree.path} className="rounded-lg border border-border/60 p-3">
                    <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="h-auto min-w-0 px-0 py-0 text-left font-mono text-xs text-foreground hover:bg-transparent"
                            onClick={() => void handleCopyPath(worktree.path)}
                            title={worktree.path}
                          >
                            <span className="truncate">{worktree.path}</span>
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon-sm"
                            className="h-6 w-6 text-muted-foreground"
                            onClick={() => void handleCopyPath(worktree.path)}
                            title={copiedPath === worktree.path ? "已复制路径" : "复制路径"}
                          >
                            {copiedPath === worktree.path ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
                          </Button>
                        </div>

                        <div className="mt-2 flex flex-wrap items-center gap-2">
                          <Badge variant="outline">{statusLabel}</Badge>
                          {worktree.is_main && <Badge variant="outline">主仓库</Badge>}
                          {worktree.task_id && (
                            <Badge variant="outline">
                              任务：{worktree.task_title ?? worktree.task_id}
                            </Badge>
                          )}
                          {!worktree.is_main && !worktree.task_id && (
                            <Badge variant="outline">孤立</Badge>
                          )}
                          {worktree.is_locked && <Badge variant="outline">锁定</Badge>}
                          {worktree.is_prunable && <Badge variant="outline">可清理</Badge>}
                          {worktree.is_bare && <Badge variant="outline">bare</Badge>}
                        </div>

                        {worktree.working_tree_summary && (
                          <div className="mt-2 text-xs text-muted-foreground">
                            {worktree.working_tree_summary}
                          </div>
                        )}
                        {worktree.lock_reason && (
                          <div className="mt-1 text-xs text-muted-foreground">
                            锁定原因：{worktree.lock_reason}
                          </div>
                        )}
                        {worktree.prunable_reason && (
                          <div className="mt-1 text-xs text-muted-foreground">
                            可清理原因：{worktree.prunable_reason}
                          </div>
                        )}
                      </div>

                      <div className="flex flex-wrap items-center gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => void loadWorktrees()}
                          disabled={loading}
                          className={getGitActionButtonClassName("neutral")}
                        >
                          刷新
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => setSelectedMergePath(worktree.path)}
                          disabled={mergeDisabled}
                          title={mergeDisabled ? "当前 worktree 不支持直接合并" : undefined}
                          className={getGitActionButtonClassName("merge")}
                        >
                          合并
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => setSelectedCommitPath(worktree.path)}
                          disabled={!hasStagedChanges || worktree.is_bare || worktree.is_prunable}
                          className={getGitActionButtonClassName("positive")}
                        >
                          提交
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => handleRemoveClick(worktree)}
                          disabled={removeDisabled}
                          title={removeDisabled ? "主仓库 worktree 不可删除" : undefined}
                          className={getGitActionButtonClassName("danger")}
                        >
                          <Trash2 className="mr-2 h-3.5 w-3.5" />
                          删除
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => setExpandedPath(isExpanded ? null : worktree.path)}
                          className={getGitActionButtonClassName("neutral")}
                        >
                          {isExpanded ? "收起" : `展开${hasChanges ? `（${worktree.working_tree_changes.length}）` : ""}`}
                        </Button>
                      </div>
                    </div>

                    {isExpanded && (
                      <div className="mt-3 border-t border-border/60 pt-3">
                        <GitChangesPanel
                          title="Worktree 文件"
                          description="当前 worktree 的实时变更文件列表，可直接在应用内预览 Diff。"
                          changes={worktree.working_tree_changes}
                          emptyLabel={
                            worktree.is_prunable
                              ? "当前 worktree 已可清理，暂无可展示的文件变更。"
                              : worktree.is_bare
                                ? "裸仓库 worktree 不提供工作区文件列表。"
                                : "当前 worktree 没有可展示的文件变更。"
                          }
                          stagingFilePath={
                            stagingFileState?.worktreePath === worktree.path
                              ? stagingFileState.filePath
                              : null
                          }
                          bulkStageAction={
                            bulkStageState?.worktreePath === worktree.path
                              ? bulkStageState.action
                              : null
                          }
                          selectedFilesStageAction={
                            selectedFilesStageState?.worktreePath === worktree.path
                              ? selectedFilesStageState.action
                              : null
                          }
                          rollbackInProgress={rollbackInProgressPath === worktree.path}
                          onToggleStage={(change) => {
                            void handleToggleStage(worktree, change);
                          }}
                          onBulkStage={(action) => {
                            void handleBulkStage(worktree, action);
                          }}
                          onStageSelected={(action, paths) => {
                            void handleStageSelected(worktree, action, paths);
                          }}
                          onRollback={(target, paths) => handleRollback(worktree, target, paths)}
                          onPreview={(change) => {
                            void handlePreview(worktree, change);
                          }}
                        />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </CollapsibleContent>
        </div>
      </Collapsible>

      {copiedPath && (
        <div className="pointer-events-none fixed bottom-6 right-6 z-50 rounded-md border border-primary/20 bg-background/95 px-3 py-2 text-sm text-foreground shadow-lg backdrop-blur-sm">
          路径已复制
        </div>
      )}

      <Suspense fallback={null}>
        <ProjectGitFilePreviewDialog
          open={selectedPreview !== null}
          loading={previewLoading}
          error={previewError}
          preview={preview}
          change={selectedPreview?.change ?? null}
          onOpenChange={(nextOpen) => {
            if (!nextOpen) {
              previewRequestIdRef.current += 1;
              setSelectedPreview(null);
              setPreview(null);
              setPreviewError(null);
              setPreviewLoading(false);
            }
          }}
        />
      </Suspense>

      <WorktreeCommitDialog
        open={selectedCommitWorktree !== null}
        projectId={projectId}
        worktree={selectedCommitWorktree}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setSelectedCommitPath(null);
          }
        }}
        onCommitted={async (message) => {
          setNotice({ tone: "success", message });
          await loadWorktrees();
          await onChanged?.();
        }}
      />

      <WorktreeMergeDialog
        open={selectedMergeWorktree !== null}
        projectId={projectId}
        worktree={selectedMergeWorktree}
        currentBranch={currentBranch}
        defaultBranch={defaultBranch}
        projectBranches={projectBranches}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setSelectedMergePath(null);
          }
        }}
        onMerged={async (message) => {
          setNotice({ tone: "success", message });
          await loadWorktrees();
          await onChanged?.();
        }}
      />

      <Dialog
        open={rollbackConfirm !== null}
        onOpenChange={(nextOpen) => {
          if (!nextOpen && !rollbackInProgressPath) {
            setRollbackConfirm(null);
          }
        }}
      >
        <DialogContent className="max-w-md" showCloseButton={!rollbackInProgressPath}>
          <DialogHeader>
            <DialogTitle>
              {rollbackConfirm?.target === "all"
                ? "确认回滚整个 Worktree"
                : `确认回滚 ${rollbackConfirm?.paths.length ?? 0} 个文件`}
            </DialogTitle>
            <DialogDescription>
              此操作将丢弃当前 worktree 中未提交的本地变更，且无法撤销。
            </DialogDescription>
          </DialogHeader>

          {rollbackConfirm?.target === "selected" && (rollbackConfirm.paths.length ?? 0) > 0 && (
            <div className="max-h-40 overflow-y-auto rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
              {rollbackConfirm?.paths.map((path) => (
                <div key={path} className="break-all py-0.5 font-mono">{path}</div>
              ))}
            </div>
          )}

          <DialogFooter className="mt-2">
            <Button
              type="button"
              variant="outline"
              disabled={rollbackInProgressPath !== null}
              onClick={() => setRollbackConfirm(null)}
            >
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={rollbackInProgressPath !== null}
              onClick={() => void confirmRollback()}
            >
              {rollbackInProgressPath ? "回滚中..." : "确认回滚"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={pendingRemove !== null}
        onOpenChange={(nextOpen) => {
          if (!nextOpen && removeMode === null) {
            setPendingRemove(null);
          }
        }}
      >
        <DialogContent className="max-w-md" showCloseButton={removeMode === null}>
          <DialogHeader>
            <DialogTitle>确认删除 Worktree</DialogTitle>
            <DialogDescription>
              将移除这个 Git worktree；如果它仍有未提交改动，可以选择强制删除。
            </DialogDescription>
          </DialogHeader>

          {pendingRemove && (
            <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
              <div className="break-all font-mono text-foreground">{pendingRemove.path}</div>
              <div className="mt-1">
                分支：{pendingRemove.branch ?? (pendingRemove.is_detached ? "detached HEAD" : "未知")}
              </div>
              <div className="mt-1">
                当前变更：{pendingRemove.working_tree_changes.length} 个文件
              </div>
            </div>
          )}

          <DialogFooter className="mt-2">
            <Button
              type="button"
              variant="outline"
              disabled={removeMode !== null}
              onClick={() => setPendingRemove(null)}
            >
              取消
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={removeMode !== null}
              onClick={() => void confirmRemove("normal")}
            >
              {removeMode === "normal" ? "删除中..." : "正常删除"}
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={removeMode !== null}
              onClick={() => void confirmRemove("force")}
            >
              {removeMode === "force" ? "强制删除中..." : "强制删除"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
