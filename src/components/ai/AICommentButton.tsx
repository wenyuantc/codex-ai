import { useState } from "react";
import { generateAIComment } from "@/lib/ai";
import { useTaskStore } from "@/stores/taskStore";
import { Bot, Loader2 } from "lucide-react";

interface AICommentButtonProps {
  taskId: string;
  taskTitle: string;
  taskDescription: string;
  taskStatus: string;
}

export function AICommentButton({
  taskId,
  taskTitle,
  taskDescription,
  taskStatus,
}: AICommentButtonProps) {
  const { addComment } = useTaskStore();
  const [loading, setLoading] = useState(false);

  const handleGenerate = async () => {
    setLoading(true);
    try {
      const comment = await generateAIComment(taskTitle, taskDescription, taskStatus);
      await addComment(taskId, comment, undefined, true);
    } catch (e) {
      console.error("Failed to generate AI comment:", e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      onClick={handleGenerate}
      disabled={loading}
      className="flex items-center gap-1 px-2 py-1 text-xs text-primary hover:bg-primary/10 rounded-md transition-colors disabled:opacity-50"
      title="AI 生成评论"
    >
      {loading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Bot className="h-3 w-3" />}
      AI 评论
    </button>
  );
}
