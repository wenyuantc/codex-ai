import type { TaskAttachment, TaskFileRef } from "@/lib/types";
import { useTranslation } from "react-i18next";
import { FolderTree, ListTodo, Loader2, MessageSquare, Paperclip, X } from "lucide-react";

import { useTaskStore } from "@/stores/taskStore";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { Button } from "@/components/ui/button";
import { CommentList } from "@/components/tasks/CommentList";
import { SubtaskList } from "@/components/tasks/SubtaskList";
import { Badge } from "@/components/ui/badge";
import { TaskAttachmentGrid } from "@/components/tasks/TaskAttachmentGrid";
import { DetailSection } from "./DetailSection";

interface TaskCollaborationPanelProps {
  taskId: string;
  attachments: TaskAttachment[];
  fileRefs: TaskFileRef[];
  deletingAttachmentId: string | null;
  attachmentLoading: boolean;
  attachmentError: string | null;
  fileRefError: string | null;
  fileRefLoading: boolean;
  deletingFileRefId: string | null;
  isTauriRuntime: boolean;
  onSelectAttachments: () => void;
  onOpenAttachment: (path: string) => void;
  onDeleteAttachment: (attachmentId: string) => void;
  onSelectFileRefs: () => void;
  onDeleteFileRef: (fileRefId: string) => void;
}

export function TaskCollaborationPanel({
  taskId,
  attachments,
  fileRefs,
  deletingAttachmentId,
  attachmentLoading,
  attachmentError,
  fileRefError,
  fileRefLoading,
  deletingFileRefId,
  isTauriRuntime,
  onSelectAttachments,
  onOpenAttachment,
  onDeleteAttachment,
  onSelectFileRefs,
  onDeleteFileRef,
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
        icon={FolderTree}
        title={t("detail.collaboration.fileRefs")}
        description={t("detail.collaboration.fileRefsDesc")}
        actions={
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={onSelectFileRefs}
            disabled={!isTauriRuntime || fileRefLoading}
            title={t("detail.collaboration.selectProjectFiles")}
          >
            {fileRefLoading ? <Loader2 className="animate-spin" /> : <FolderTree />}
            {t("detail.collaboration.selectProjectFiles")}
          </Button>
        }
      >
        <div className="space-y-3">
          {fileRefError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {fileRefError}
            </div>
          )}
          {fileRefs.length === 0 ? (
            <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
              {t("detail.collaboration.noFileRefs")}
            </div>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {fileRefs.map((fileRef) => (
                <Badge
                  key={fileRef.id}
                  variant="outline"
                  className="max-w-full gap-1 pr-1 font-mono"
                >
                  <span className="truncate" title={fileRef.relative_path}>
                    {fileRef.relative_path}
                  </span>
                  {deletingFileRefId !== fileRef.id && (
                    <button
                      type="button"
                      onClick={() => onDeleteFileRef(fileRef.id)}
                      className="rounded-full p-0.5 hover:bg-muted"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  )}
                </Badge>
              ))}
            </div>
          )}
        </div>
      </DetailSection>

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
