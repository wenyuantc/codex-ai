import type { TaskAttachment } from "@/lib/types";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation("tasks");
  const subtasks = useTaskStore((state) => state.subtasks[taskId]);
  const comments = useTaskStore((state) => state.comments[taskId]);
  const subtaskItems = subtasks ?? [];
  const subtaskDoneCount = subtaskItems.filter((item) => item.status === "completed").length;
  const commentCount = comments?.length ?? 0;

  return (
    <div className="space-y-4">
      <DetailSection
        icon={Paperclip}
        title={t("detail.collaboration.attachments")}
        description={t("detail.collaboration.attachmentsDesc")}
        actions={
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={onSelectAttachments}
            disabled={!isTauriRuntime || attachmentLoading}
            title={
              isTauriRuntime
                ? t("detail.collaboration.upload")
                : t("detail.collaboration.uploadDesktopOnly")
            }
          >
            {attachmentLoading ? <Loader2 className="animate-spin" /> : <Paperclip />}
            {t("detail.collaboration.add")}
          </Button>
        }
      >
        <div className="space-y-3">
          {!isTauriRuntime && (
            <div className="rounded-md border border-border/60 bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
              {t("detail.collaboration.unsupportedEnv")}
            </div>
          )}

          {attachmentError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {attachmentError}
            </div>
          )}

          <ErrorBoundary
            fallbackTitle={t("detail.collaboration.fallbackTitle")}
            fallbackDescription={t("detail.collaboration.fallbackDesc")}
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
              emptyText={t("detail.collaboration.empty")}
            />
          </ErrorBoundary>
        </div>
      </DetailSection>

      <DetailSection
        icon={ListTodo}
        title={
          <>
            {t("detail.collaboration.subtasks")}
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
            {t("detail.collaboration.comments")}
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
