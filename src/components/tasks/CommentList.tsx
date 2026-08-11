import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useTaskStore } from "@/stores/taskStore";
import { CommentItem } from "./CommentItem";
import { Send } from "lucide-react";

interface CommentListProps {
  taskId: string;
  /** Hide the built-in heading (used when a parent section already renders one). */
  hideHeader?: boolean;
}

export function CommentList({ taskId, hideHeader = false }: CommentListProps) {
  const { t } = useTranslation("tasks");
  const { comments, fetchComments, addComment } = useTaskStore();
  const [newContent, setNewContent] = useState("");

  useEffect(() => {
    fetchComments(taskId);
  }, [taskId, fetchComments]);

  const items = comments[taskId] ?? [];

  const handleAdd = async () => {
    if (!newContent.trim()) return;
    await addComment(taskId, newContent.trim());
    setNewContent("");
  };

  return (
    <div>
      {!hideHeader && (
        <h3 className="text-xs font-medium text-muted-foreground mb-2">
          {t("detail.collaboration.comments")} ({items.length})
        </h3>
      )}

      {items.length > 0 && (
        <div className="space-y-2 mb-3">
          {items.map((comment) => (
            <CommentItem key={comment.id} comment={comment} />
          ))}
        </div>
      )}

      <div className="flex gap-1.5">
        <input
          type="text"
          value={newContent}
          onChange={(e) => setNewContent(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          placeholder={t("detail.collaboration.addCommentPlaceholder")}
          className="flex-1 text-xs border border-input rounded px-2 py-1.5 bg-background"
        />
        <button
          onClick={handleAdd}
          disabled={!newContent.trim()}
          className="p-1.5 text-muted-foreground hover:text-foreground disabled:opacity-50"
        >
          <Send className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
