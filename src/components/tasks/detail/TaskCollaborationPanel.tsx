import type { TaskAttachment } from "@/lib/types";
import { ListTodo, Loader2, MessageSquare, Paperclip } from "lucide-react";

import { useTaskStore } from "@/stores/taskStore";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { Button } from "@/components/ui/button";
import { CommentList } from "@/components/tasks/CommentList";
import { SubtaskList } from "@/components/tasks/SubtaskList";
import { TaskAttachmentGrid } from "@/components/tasks/TaskAttachmentGrid";
import { DetailSection } from "./DetailSection";

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
  const subtasks = useTaskStore((state) => state.subtasks[taskId]);
  const comments = useTaskStore((state) => state.comments[taskId]);
  const subtaskItems = subtasks ?? [];
  const subtaskDoneCount = subtaskItems.filter((item) => item.status === "completed").length;
  const commentCount = comments?.length ?? 0;

  return (
    <div className="space-y-4">
      <DetailSection
        icon={Paperclip}
        title="附件"
        description="当前任务的附件会随任务上下文保留，图片会在每次启动和续聊时自动附带给 Codex。"
        actions={
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={onSelectAttachments}
            disabled={!isTauriRuntime || attachmentLoading}
            title={isTauriRuntime ? "上传附件" : "仅桌面端支持上传附件"}
          >
            {attachmentLoading ? <Loader2 className="animate-spin" /> : <Paperclip />}
            添加附件
          </Button>
        }
      >
        <div className="space-y-3">
          {!isTauriRuntime && (
            <div className="rounded-md border border-border/60 bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
              当前环境不支持任务附件上传，请在桌面端使用该功能。
            </div>
          )}

          {attachmentError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {attachmentError}
            </div>
          )}

          <ErrorBoundary
            fallbackTitle="附件区渲染失败"
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
                onOpen: isTauriRuntime ? () => onOpenAttachment(attachment.stored_path) : undefined,
                onRemove: () => onDeleteAttachment(attachment.id),
              }))}
              emptyText="当前任务还没有附件"
            />
          </ErrorBoundary>
        </div>
      </DetailSection>

      <DetailSection
        icon={ListTodo}
        title={
          <>
            子任务
            {subtaskItems.length > 0 && (
              <span className="text-[11px] font-normal text-muted-foreground">
                {subtaskDoneCount}/{subtaskItems.length}
              </span>
            )}
          </>
        }
        contentClassName="mt-2"
      >
        <SubtaskList taskId={taskId} hideHeader />
      </DetailSection>

      <DetailSection
        icon={MessageSquare}
        title={
          <>
            评论
            {commentCount > 0 && (
              <span className="text-[11px] font-normal text-muted-foreground">{commentCount}</span>
            )}
          </>
        }
        contentClassName="mt-2"
      >
        <CommentList taskId={taskId} hideHeader />
      </DetailSection>
    </div>
  );
}
