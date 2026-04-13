import { useEffect, useState } from "react";
import type { Task } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ContinueConversationDialogProps {
  open: boolean;
  task: Task | null;
  submitting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (prompt: string) => Promise<void> | void;
}

export function ContinueConversationDialog({
  open,
  task,
  submitting = false,
  onOpenChange,
  onConfirm,
}: ContinueConversationDialogProps) {
  const [prompt, setPrompt] = useState("");

  useEffect(() => {
    if (open) {
      setPrompt("");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,42rem)] max-w-[min(96vw,42rem)] sm:max-w-[min(96vw,42rem)]">
        <DialogHeader>
          <DialogTitle>继续对话</DialogTitle>
          <DialogDescription>
            向任务“{task?.title ?? ""}”上一次 Codex session 发送新的续聊内容。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <label className="text-sm font-medium text-foreground" htmlFor="continue-conversation-prompt">
            对话内容
          </label>
          <textarea
            id="continue-conversation-prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="输入想继续追问或继续执行的内容..."
            className="min-h-32 w-full resize-y rounded-md border border-input bg-background p-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
            disabled={submitting}
          />
        </div>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            取消
          </Button>
          <Button
            type="button"
            onClick={() => void onConfirm(prompt.trim())}
            disabled={submitting || !prompt.trim()}
          >
            {submitting ? "执行中..." : "执行"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
