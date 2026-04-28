import { lazy, Suspense } from "react";
import { Loader2, Play, RefreshCw, Save } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const MonacoMarkdownEditor = lazy(() => import("./detail/MonacoMarkdownEditor").then((module) => ({
  default: module.MonacoMarkdownEditor,
})));

interface CoordinatorPlanDialogProps {
  open: boolean;
  coordinatorName?: string;
  plan: string;
  loading: boolean;
  saving: boolean;
  executing: boolean;
  error: string | null;
  canExecute?: boolean;
  onOpenChange: (open: boolean) => void;
  onPlanChange: (value: string) => void;
  onExecute: () => void;
  onRegenerate: () => void;
  onSave: () => void;
}

function MonacoEditorFallback() {
  return (
    <div className="flex h-[360px] items-center justify-center rounded-md border border-dashed text-xs text-muted-foreground">
      正在加载编辑器...
    </div>
  );
}

export function CoordinatorPlanDialog({
  open,
  coordinatorName,
  plan,
  loading,
  saving,
  executing,
  error,
  canExecute = true,
  onOpenChange,
  onPlanChange,
  onExecute,
  onRegenerate,
  onSave,
}: CoordinatorPlanDialogProps) {
  const hasPlan = plan.trim().length > 0;
  const busy = loading || saving || executing;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(94vw,52rem)] max-w-[min(94vw,52rem)] sm:max-w-[min(94vw,52rem)]">
        <DialogHeader>
          <DialogTitle>协调员执行计划</DialogTitle>
          <DialogDescription>
            {coordinatorName ? `由 ${coordinatorName} 生成计划，确认后交给指派员工执行。` : "确认后交给指派员工执行。"}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          {loading && !hasPlan ? (
            <div className="flex min-h-52 items-center justify-center rounded-md border border-dashed bg-muted/30 text-sm text-muted-foreground">
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              正在生成计划中…
            </div>
          ) : (
            <Suspense fallback={<MonacoEditorFallback />}>
              <MonacoMarkdownEditor
                value={plan}
                onChange={onPlanChange}
                readOnly={busy}
                className="h-[360px]"
                placeholder="协调员生成的计划会显示在这里，可在执行前编辑。"
              />
            </Suspense>
          )}

          {error && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          )}

          <div className="flex flex-wrap justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              disabled={busy}
              className="rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
            >
              取消
            </button>
            <button
              type="button"
              onClick={onSave}
              disabled={busy || !hasPlan}
              className="flex items-center gap-1 rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
            >
              {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
              保存计划
            </button>
            <button
              type="button"
              onClick={onRegenerate}
              disabled={busy}
              className="flex items-center gap-1 rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
            >
              {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
              重新生成计划
            </button>
            <button
              type="button"
              onClick={onExecute}
              disabled={busy || !hasPlan || !canExecute}
              className="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {executing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
              执行
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
