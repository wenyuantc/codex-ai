import type { CodexSessionFileChangeDetail } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  getExecutionChangeCaptureModeDescription,
  getExecutionChangeCaptureModeLabel,
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getExecutionDiffLineClassName,
  getExecutionSnapshotStatusLabel,
} from "./taskDetailViewHelpers";

interface TaskExecutionChangeDetailDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  detail: CodexSessionFileChangeDetail | null;
  onOpenChange: (open: boolean) => void;
}

function CodePreview({ text, diffMode = false }: { text: string; diffMode?: boolean }) {
  const lines = text.split(/\r?\n/);

  return (
    <ScrollArea className="h-[28rem] overflow-hidden rounded-md border bg-background/80">
      <div className="p-3 font-mono text-xs leading-5">
        {lines.map((line, index) => (
          <div
            key={`${index}-${line}`}
            className={`whitespace-pre-wrap break-all ${diffMode ? getExecutionDiffLineClassName(line) : "text-foreground"}`}
          >
            {line || " "}
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}

function SnapshotMeta({
  label,
  status,
  truncated,
}: {
  label: string;
  status: "text" | "missing" | "binary" | "unavailable";
  truncated: boolean;
}) {
  return (
    <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs">
      <div className="font-medium text-foreground">{label}</div>
      <div className="mt-1 text-muted-foreground">
        {getExecutionSnapshotStatusLabel(status)}
        {truncated ? " · 已截断" : ""}
      </div>
    </div>
  );
}

export function TaskExecutionChangeDetailDialog({
  open,
  loading,
  error,
  detail,
  onOpenChange,
}: TaskExecutionChangeDetailDialogProps) {
  const hasBeforeText = detail?.before_status === "text" && detail.before_text !== null;
  const hasAfterText = detail?.after_status === "text" && detail.after_text !== null;
  const hasDiffText = detail?.diff_text !== null && detail?.diff_text !== undefined;
  const defaultTab =
    detail?.change.change_type === "added" && hasAfterText
      ? "after"
      : hasDiffText
        ? "diff"
        : hasBeforeText
          ? "before"
          : "after";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,72rem)] max-w-[min(96vw,72rem)] sm:max-w-[min(96vw,72rem)]">
        <DialogHeader>
          <DialogTitle>
            {detail
              ? `${getExecutionChangeTypeLabel(detail.change.change_type)} ${detail.change.path}`
              : "文件变更详情"}
          </DialogTitle>
          <DialogDescription>
            查看该次执行会话记录下来的文件快照与 diff 预览。旧记录可能只有文件路径，没有文本快照。
          </DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            正在加载文件变更详情...
          </div>
        ) : error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive">
            {error}
          </div>
        ) : !detail ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            暂无可展示的文件详情。
          </div>
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={`rounded-md border px-2 py-1 text-xs font-medium ${getExecutionChangeTypeClassName(detail.change.change_type)}`}
              >
                {getExecutionChangeTypeLabel(detail.change.change_type)}
              </span>
              <span className="rounded-md border border-border/70 bg-muted/30 px-2 py-1 text-xs text-muted-foreground">
                {getExecutionChangeCaptureModeLabel(detail.change.capture_mode)}
              </span>
            </div>

            <div className="rounded-md border border-border/70 bg-muted/20 p-3 text-xs text-muted-foreground">
              {getExecutionChangeCaptureModeDescription(detail.change.capture_mode)}
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">绝对路径</div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.absolute_path ?? "未记录"}
                </div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">工作目录</div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.working_dir ?? "未记录"}
                </div>
              </div>
              {detail.change.previous_path && (
                <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs md:col-span-2">
                  <div className="font-medium text-foreground">原路径</div>
                  <div className="mt-1 whitespace-pre-wrap break-all font-mono text-muted-foreground">
                    {detail.change.previous_path}
                    {detail.previous_absolute_path ? `\n${detail.previous_absolute_path}` : ""}
                  </div>
                </div>
              )}
            </div>

            {detail.snapshot_message && (
              <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
                {detail.snapshot_message}
              </div>
            )}

            {detail.snapshot_status === "ready" && (
              <>
                <div className="grid gap-2 md:grid-cols-2">
                  <SnapshotMeta
                    label="变更前快照"
                    status={detail.before_status}
                    truncated={detail.before_truncated}
                  />
                  <SnapshotMeta
                    label="变更后快照"
                    status={detail.after_status}
                    truncated={detail.after_truncated}
                  />
                </div>

                {detail.diff_truncated && (
                  <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                    当前 diff 预览可能已截断，或基于截断后的文本快照生成。
                  </div>
                )}

                {hasDiffText || hasBeforeText || hasAfterText ? (
                  <Tabs defaultValue={defaultTab}>
                    <TabsList className="grid w-full grid-cols-3">
                      <TabsTrigger value="diff" disabled={!hasDiffText}>
                        Diff 预览
                      </TabsTrigger>
                      <TabsTrigger value="before" disabled={!hasBeforeText}>
                        变更前
                      </TabsTrigger>
                      <TabsTrigger value="after" disabled={!hasAfterText}>
                        变更后
                      </TabsTrigger>
                    </TabsList>

                    <TabsContent value="diff" className="space-y-2">
                      {hasDiffText ? (
                        <CodePreview text={detail.diff_text ?? ""} diffMode />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          当前记录没有可展示的文本 diff。
                        </div>
                      )}
                    </TabsContent>

                    <TabsContent value="before" className="space-y-2">
                      {hasBeforeText ? (
                        <CodePreview text={detail.before_text ?? ""} />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          当前记录没有保存变更前的文本内容。
                        </div>
                      )}
                    </TabsContent>

                    <TabsContent value="after" className="space-y-2">
                      {hasAfterText ? (
                        <CodePreview text={detail.after_text ?? ""} />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          当前记录没有保存变更后的文本内容。
                        </div>
                      )}
                    </TabsContent>
                  </Tabs>
                ) : (
                  <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                    这条记录没有可展示的文本内容，通常是二进制文件、不可读文件，或那次会话没有保留文本快照。
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
