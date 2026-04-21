import { useEffect, useState } from "react";

import type { ProjectGitCommit, ProjectGitCommitDetail } from "@/lib/types";
import { formatDate } from "@/lib/utils";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getExecutionDiffLineClassName,
} from "@/components/tasks/detail/taskDetailViewHelpers";

interface ProjectGitCommitDetailDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  detail: ProjectGitCommitDetail | null;
  commit: ProjectGitCommit | null;
  onOpenChange: (open: boolean) => void;
}

type CommitDetailTabValue = "diff" | "files";

function DiffPreview({ text }: { text: string }) {
  const lines = text.split(/\r?\n/);

  return (
    <ScrollArea className="h-[30rem] overflow-hidden rounded-md border bg-background/80">
      <div className="p-3 font-mono text-xs leading-5">
        {lines.map((line, index) => (
          <div
            key={`${index}-${line}`}
            className={`whitespace-pre-wrap break-all ${getExecutionDiffLineClassName(line)}`}
          >
            {line || " "}
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}

export function ProjectGitCommitDetailDialog({
  open,
  loading,
  error,
  detail,
  commit,
  onOpenChange,
}: ProjectGitCommitDetailDialogProps) {
  const displayTitle = detail?.subject ?? commit?.subject ?? "提交详情";
  const hasDiffText = detail?.diff_text !== null && detail?.diff_text !== undefined;
  const hasChangedFiles = (detail?.changed_files.length ?? 0) > 0;
  const getDefaultTab = (): CommitDetailTabValue => (hasDiffText ? "diff" : "files");
  const [activeTab, setActiveTab] = useState<CommitDetailTabValue>(getDefaultTab);

  useEffect(() => {
    if (!open) {
      return;
    }
    setActiveTab(getDefaultTab());
  }, [detail?.sha, hasChangedFiles, hasDiffText, open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,78rem)] max-w-[min(96vw,78rem)] sm:max-w-[min(96vw,78rem)]">
        <DialogHeader>
          <DialogTitle>{displayTitle}</DialogTitle>
          <DialogDescription>
            查看本次提交的元信息、改动文件列表与 diff 预览。
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            正在加载提交详情...
          </div>
        ) : error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive">
            {error}
          </div>
        ) : !detail ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            暂无可展示的提交详情。
          </div>
        ) : (
          <div className="space-y-4">
            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">提交 SHA</div>
                <div className="mt-1 break-all font-mono text-muted-foreground">{detail.sha}</div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">提交作者</div>
                <div className="mt-1 text-muted-foreground">
                  {detail.author_name}
                  {detail.author_email ? ` · ${detail.author_email}` : ""}
                </div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">提交时间</div>
                <div className="mt-1 text-muted-foreground">{formatDate(detail.authored_at)}</div>
              </div>
            </div>

            {detail.body && (
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-3 text-sm text-muted-foreground whitespace-pre-wrap break-all">
                {detail.body}
              </div>
            )}

            {detail.diff_truncated && (
              <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                当前提交 diff 较长，预览内容已截断。
              </div>
            )}

            <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as CommitDetailTabValue)}>
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="diff" disabled={!hasDiffText}>
                  Diff 预览
                </TabsTrigger>
                <TabsTrigger value="files" disabled={!hasChangedFiles}>
                  变更文件
                </TabsTrigger>
              </TabsList>

              <TabsContent value="diff" className="space-y-2">
                {hasDiffText ? (
                  <DiffPreview text={detail.diff_text ?? ""} />
                ) : (
                  <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                    当前提交没有可展示的文本 diff。
                  </div>
                )}
              </TabsContent>

              <TabsContent value="files" className="space-y-2">
                {hasChangedFiles ? (
                  <ScrollArea className="h-[30rem] overflow-hidden rounded-md border bg-background/80">
                    <div className="space-y-2 p-3">
                      {detail.changed_files.map((change) => (
                        <div
                          key={`${change.change_type}:${change.previous_path ?? ""}:${change.path}`}
                          className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs"
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <span
                              className={`rounded-md border px-2 py-1 font-medium ${getExecutionChangeTypeClassName(change.change_type)}`}
                            >
                              {getExecutionChangeTypeLabel(change.change_type)}
                            </span>
                            <span className="break-all font-mono text-foreground">{change.path}</span>
                          </div>
                          {change.previous_path && (
                            <div className="mt-1 break-all font-mono text-muted-foreground">
                              原路径：{change.previous_path}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  </ScrollArea>
                ) : (
                  <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                    当前提交没有可展示的变更文件列表。
                  </div>
                )}
              </TabsContent>
            </Tabs>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
