import { useEffect, useMemo, useState } from "react";

import { countStageableGitFiles, countStagedGitFiles } from "@/lib/gitWorkingTree";
import type { ProjectGitWorkingTreeChange } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  getWorkingTreeChangeClassName,
  getWorkingTreeChangeLabel,
  getWorkingTreeStageStatusClassName,
  getWorkingTreeStageStatusLabel,
} from "@/components/git/gitHelpers";

export interface GitChangesPanelProps {
  title: string;
  description?: string;
  changes: ProjectGitWorkingTreeChange[];
  maxDisplay?: number;
  emptyLabel?: string;
  stagingFilePath: string | null;
  bulkStageAction: "stage_all" | "unstage_all" | null;
  selectedFilesStageAction: "stage" | "unstage" | null;
  rollbackInProgress: boolean;
  onToggleStage: (change: ProjectGitWorkingTreeChange) => void;
  onBulkStage: (action: "stage_all" | "unstage_all") => void;
  onStageSelected: (action: "stage" | "unstage", paths: string[]) => void;
  onRollback: (target: "selected" | "all", paths?: string[]) => void;
  onPreview: (change: ProjectGitWorkingTreeChange) => void;
}

export function GitChangesPanel({
  title,
  description,
  changes,
  maxDisplay = 20,
  emptyLabel = "当前没有可展示的工作区文件变更。",
  stagingFilePath,
  bulkStageAction,
  selectedFilesStageAction,
  rollbackInProgress,
  onToggleStage,
  onBulkStage,
  onStageSelected,
  onRollback,
  onPreview,
}: GitChangesPanelProps) {
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const visibleChanges = useMemo(
    () => changes.slice(0, Math.max(1, maxDisplay)),
    [changes, maxDisplay],
  );
  const hasStageableFiles = useMemo(
    () => countStageableGitFiles(changes) > 0,
    [changes],
  );
  const hasStagedFiles = useMemo(
    () => countStagedGitFiles(changes) > 0,
    [changes],
  );
  const selectedPaths = useMemo(
    () => visibleChanges.filter((change) => selectedFiles.has(change.path)).map((change) => change.path),
    [selectedFiles, visibleChanges],
  );
  const allVisibleSelected = visibleChanges.length > 0 && visibleChanges.every((change) => selectedFiles.has(change.path));

  useEffect(() => {
    const visiblePathSet = new Set(visibleChanges.map((change) => change.path));
    setSelectedFiles((current) => {
      const next = new Set(Array.from(current).filter((path) => visiblePathSet.has(path)));
      if (next.size === current.size && Array.from(next).every((path) => current.has(path))) {
        return current;
      }
      return next;
    });
  }, [visibleChanges]);

  return (
    <div>
      <div className="mb-2 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          {visibleChanges.length > 0 && (
            <input
              type="checkbox"
              className="h-3.5 w-3.5 cursor-pointer rounded"
              checked={allVisibleSelected}
              onChange={(event) => {
                if (event.target.checked) {
                  setSelectedFiles(new Set(visibleChanges.map((change) => change.path)));
                  return;
                }
                setSelectedFiles(new Set());
              }}
              title="全选/取消全选"
            />
          )}
          <div>
            <h4 className="text-sm font-medium">{title}</h4>
            {description && (
              <p className="text-[11px] text-muted-foreground">{description}</p>
            )}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">{changes.length} 条</span>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!hasStageableFiles || bulkStageAction !== null}
            onClick={() => onBulkStage("stage_all")}
          >
            {bulkStageAction === "stage_all" ? "暂存中..." : "全部暂存"}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!hasStagedFiles || bulkStageAction !== null}
            onClick={() => onBulkStage("unstage_all")}
          >
            {bulkStageAction === "unstage_all" ? "取消中..." : "全部取消暂存"}
          </Button>
          {selectedPaths.length > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={selectedFilesStageAction !== null || rollbackInProgress}
              onClick={() => onStageSelected("stage", selectedPaths)}
            >
              {selectedFilesStageAction === "stage" ? "暂存中..." : `暂存选中 (${selectedPaths.length})`}
            </Button>
          )}
          {selectedPaths.length > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={selectedFilesStageAction !== null || rollbackInProgress}
              onClick={() => onStageSelected("unstage", selectedPaths)}
            >
              {selectedFilesStageAction === "unstage" ? "取消中..." : `取消暂存选中 (${selectedPaths.length})`}
            </Button>
          )}
          {selectedPaths.length > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={rollbackInProgress}
              onClick={() => onRollback("selected", selectedPaths)}
              className="border-orange-500/50 text-orange-700 hover:bg-orange-50 hover:text-orange-800"
            >
              回滚选中 ({selectedPaths.length})
            </Button>
          )}
          {changes.length > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={rollbackInProgress}
              onClick={() => onRollback("all")}
              className="border-red-500/50 text-red-700 hover:bg-red-50 hover:text-red-800"
            >
              全局回滚
            </Button>
          )}
        </div>
      </div>

      {changes.length === 0 ? (
        <div className="rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
          {emptyLabel}
        </div>
      ) : (
        <div className="space-y-2">
          {visibleChanges.map((change) => (
            <div
              key={`${change.change_type}:${change.previous_path ?? ""}:${change.path}`}
              className={`rounded-md border border-border/60 bg-secondary/20 px-3 py-2 text-xs ${change.can_open_file ? "cursor-pointer transition-colors hover:border-primary/40 hover:bg-secondary/30" : ""}`}
              role={change.can_open_file ? "button" : undefined}
              tabIndex={change.can_open_file ? 0 : undefined}
              onClick={() => {
                if (change.can_open_file) {
                  onPreview(change);
                }
              }}
              onKeyDown={(event) => {
                if (!change.can_open_file) {
                  return;
                }
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onPreview(change);
                }
              }}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex min-w-0 flex-wrap items-center gap-2">
                  <input
                    type="checkbox"
                    className="h-3.5 w-3.5 cursor-pointer rounded"
                    checked={selectedFiles.has(change.path)}
                    onChange={(event) => {
                      event.stopPropagation();
                      setSelectedFiles((current) => {
                        const next = new Set(current);
                        if (event.target.checked) {
                          next.add(change.path);
                        } else {
                          next.delete(change.path);
                        }
                        return next;
                      });
                    }}
                    onClick={(event) => event.stopPropagation()}
                  />
                  <span
                    className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getWorkingTreeChangeClassName(change.change_type)}`}
                  >
                    {getWorkingTreeChangeLabel(change.change_type)}
                  </span>
                  <span
                    className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getWorkingTreeStageStatusClassName(change.stage_status)}`}
                  >
                    {getWorkingTreeStageStatusLabel(change.stage_status)}
                  </span>
                  <span className="break-all font-mono text-foreground">{change.path}</span>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={(event) => {
                      event.stopPropagation();
                      onPreview(change);
                    }}
                    disabled={!change.can_open_file}
                    title={
                      change.can_open_file
                        ? "使用内置代码预览浏览当前文件"
                        : change.change_type === "deleted"
                          ? "已删除文件无法直接浏览"
                          : "当前文件暂不可浏览"
                    }
                  >
                    浏览文件
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleStage(change);
                    }}
                    disabled={stagingFilePath === change.path || bulkStageAction !== null}
                  >
                    {stagingFilePath === change.path
                      ? (change.stage_status === "staged" || change.stage_status === "partially_staged"
                        ? "取消中..."
                        : "暂存中...")
                      : (change.stage_status === "staged" || change.stage_status === "partially_staged"
                        ? "取消暂存"
                        : "暂存")}
                  </Button>
                </div>
              </div>
              {change.previous_path && (
                <div className="mt-1 text-[11px] text-muted-foreground">
                  原路径：<span className="break-all font-mono">{change.previous_path}</span>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
