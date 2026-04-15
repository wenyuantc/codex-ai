import type { TaskAttachment } from "@/lib/types";
import { ImagePlus, Loader2 } from "lucide-react";

import { ErrorBoundary } from "@/components/ErrorBoundary";
import { CommentList } from "@/components/tasks/CommentList";
import { SubtaskList } from "@/components/tasks/SubtaskList";
import { TaskAttachmentGrid } from "@/components/tasks/TaskAttachmentGrid";

interface TaskCollaborationPanelProps {
  taskId: string;
  attachments: TaskAttachment[];
  deletingAttachmentId: string | null;
  attachmentLoading: boolean;
  attachmentError: string | null;
  isTauriRuntime: boolean;
  onSelectAttachments: () => void;
  onOpenAttachment: (path: string) => void;
  onDeleteAttachment: (attachmentId: string) => void;
}

export function TaskCollaborationPanel({
  taskId,
  attachments,
  deletingAttachmentId,
  attachmentLoading,
  attachmentError,
  isTauriRuntime,
  onSelectAttachments,
  onOpenAttachment,
  onDeleteAttachment,
}: TaskCollaborationPanelProps) {
  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              图片附件
            </label>
            <p className="text-[11px] text-muted-foreground">
              当前任务的图片会在每次启动和续聊时自动附带给 Codex。
            </p>
          </div>
          <button
            type="button"
            onClick={onSelectAttachments}
            disabled={!isTauriRuntime || attachmentLoading}
            className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
            title={isTauriRuntime ? "上传图片" : "仅桌面端支持上传图片"}
          >
            {attachmentLoading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <ImagePlus className="h-3.5 w-3.5" />
            )}
            添加图片
          </button>
        </div>

        {!isTauriRuntime && (
          <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
            当前环境不支持任务图片上传，请在桌面端使用该功能。
          </div>
        )}

        {attachmentError && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {attachmentError}
          </div>
        )}

        <ErrorBoundary
          fallbackTitle="图片附件区渲染失败"
          fallbackDescription="附件数据已保留，但缩略图区域发生了运行时异常。"
        >
          <TaskAttachmentGrid
            items={attachments.map((attachment) => ({
              id: attachment.id,
              name: attachment.original_name,
              path: attachment.stored_path,
              fileSize: attachment.file_size,
              mimeType: attachment.mime_type,
              removable: deletingAttachmentId !== attachment.id,
              onOpen: isTauriRuntime
                ? () => onOpenAttachment(attachment.stored_path)
                : undefined,
              onRemove: () => onDeleteAttachment(attachment.id),
            }))}
            emptyText="当前任务还没有图片"
          />
        </ErrorBoundary>
      </div>

      <div className="rounded-md border border-border/70 bg-background/60 p-3">
        <SubtaskList taskId={taskId} />
      </div>

      <div className="rounded-md border border-border/70 bg-background/60 p-3">
        <CommentList taskId={taskId} />
      </div>
    </div>
  );
}
