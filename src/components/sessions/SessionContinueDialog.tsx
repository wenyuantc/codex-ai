import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { CodexSessionListItem } from "@/lib/types";

interface SessionContinueDialogProps {
  open: boolean;
  session: CodexSessionListItem | null;
  submitting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (prompt: string) => Promise<void> | void;
}

export function SessionContinueDialog({
  open,
  session,
  submitting = false,
  onOpenChange,
  onConfirm,
}: SessionContinueDialogProps) {
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
            向 Session “{session?.display_name ?? "未命名会话"}” 发送新的续聊内容。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <div className="rounded-lg border border-border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
            <div className="font-mono">session id: {session?.session_id ?? "暂无"}</div>
            <div className="mt-1">关联任务：{session?.task_title ?? "无关联任务"}</div>
          </div>
          <label className="text-sm font-medium text-foreground" htmlFor="session-continue-prompt">
            对话内容
          </label>
          <textarea
            id="session-continue-prompt"
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
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
            {submitting ? "执行中..." : "执行并查看日志"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
