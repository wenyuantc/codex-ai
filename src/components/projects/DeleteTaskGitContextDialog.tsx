import type { TaskGitContext } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface DeleteTaskGitContextDialogProps {
  open: boolean;
  context: TaskGitContext | null;
  deleting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void> | void;
}

export function DeleteTaskGitContextDialog({
  open,
  context,
  deleting = false,
  onOpenChange,
  onConfirm,
}: DeleteTaskGitContextDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!deleting}>
        <DialogHeader>
          <DialogTitle>确认删除 Git 上下文记录</DialogTitle>
          <DialogDescription>
            确认删除任务分支“{context?.task_branch ?? "未命名分支"}”对应的 Git 上下文记录吗？
            该操作会移除这条失效记录，且无法恢复。
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>目标分支：{context?.target_branch ?? "未设置"}</div>
          <div className="mt-1">当前任务 worktree 已不存在，这条记录可以直接删除。</div>
        </div>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={deleting}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={() => void onConfirm()}
            disabled={deleting}
          >
            {deleting ? "删除中..." : "确认删除"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
