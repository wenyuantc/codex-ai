import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ReviewFixConfirmDialogProps {
  open: boolean;
  sourceTaskTitle: string;
  assigneeName: string;
  creating?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void> | void;
}

export function ReviewFixConfirmDialog({
  open,
  sourceTaskTitle,
  assigneeName,
  creating = false,
  onOpenChange,
  onConfirm,
}: ReviewFixConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!creating}>
        <DialogHeader>
          <DialogTitle>创建修复任务</DialogTitle>
          <DialogDescription>
            将基于任务“{sourceTaskTitle}”的审核结果创建一个新的修复任务，并立即交给
            {assigneeName} 运行。确认后会保留当前审核任务，不会覆盖原任务内容。
          </DialogDescription>
        </DialogHeader>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={creating}
          >
            取消
          </Button>
          <Button type="button" onClick={() => void onConfirm()} disabled={creating}>
            {creating ? "创建并运行中..." : "确认修复"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
