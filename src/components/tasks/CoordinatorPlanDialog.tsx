import { lazy, Suspense, useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  Eraser,
  GitBranch,
  Loader2,
  Play,
  RefreshCw,
  Save,
  Terminal,
} from "lucide-react";

import type { Employee, TaskAutomationState, TaskPipelineStep } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SessionLogDialog } from "@/components/sessions/SessionLogDialog";
import { getAiLogColor } from "./detail/taskDetailViewHelpers";
import { TaskPipelineProgress } from "./detail/TaskPipelineProgress";

const MonacoMarkdownEditor = lazy(() =>
  import("./detail/MonacoMarkdownEditor").then((module) => ({
    default: module.MonacoMarkdownEditor,
  })),
);

interface CoordinatorPlanDialogProps {
  open: boolean;
  coordinatorName?: string;
  plan: string;
  loading: boolean;
  saving: boolean;
  executing: boolean;
  error: string | null;
  canExecute?: boolean;
  terminalLogs: string[];
  terminalVisible: boolean;
  pipelineSteps?: TaskPipelineStep[];
  pipelineAutomation?: Pick<
    TaskAutomationState,
    "pipeline_active" | "pipeline_step_index" | "phase"
  > | null;
  pipelineLoading?: boolean;
  pipelineActionLoading?: boolean;
  pipelineError?: string | null;
  pipelineNotice?: string | null;
  employees?: Employee[];
  projectId?: string;
  canStartPipeline?: boolean;
  /** Task is running execution / pipeline — lock plan mutation and start actions. */
  actionsLocked?: boolean;
  taskTitle?: string;
  onOpenChange: (open: boolean) => void;
  onPlanChange: (value: string) => void;
  onExecute: () => void;
  onStartPipeline?: () => void;
  onRetryPipeline?: () => void;
  onAbortPipeline?: () => void;
  onManualRunPipelineStep?: (step: TaskPipelineStep) => void;
  onRefreshPipeline?: () => void;
  onPipelineEmployeeChange?: (stepId: string, employeeId: string) => void;
  onRegenerate: () => void;
  onSave: () => void;
  onToggleTerminal: () => void;
  onClearTerminal: () => void;
}

interface StepLogTarget {
  sessionRecordId: string;
  stepTitle: string;
  employeeName: string | null;
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
  terminalLogs,
  terminalVisible,
  pipelineSteps = [],
  pipelineAutomation = null,
  pipelineLoading = false,
  pipelineActionLoading = false,
  pipelineError = null,
  pipelineNotice = null,
  employees = [],
  projectId,
  canStartPipeline = true,
  actionsLocked = false,
  taskTitle,
  onOpenChange,
  onPlanChange,
  onExecute,
  onStartPipeline,
  onRetryPipeline,
  onAbortPipeline,
  onManualRunPipelineStep,
  onRefreshPipeline,
  onPipelineEmployeeChange,
  onRegenerate,
  onSave,
  onToggleTerminal,
  onClearTerminal,
}: CoordinatorPlanDialogProps) {
  const hasPlan = plan.trim().length > 0;
  const busy = loading || saving || executing || pipelineActionLoading;
  const planActionsDisabled = busy || actionsLocked;
  const terminalBottomRef = useRef<HTMLDivElement>(null);
  const [stepLogTarget, setStepLogTarget] = useState<StepLogTarget | null>(null);

  useEffect(() => {
    if (terminalVisible) {
      terminalBottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [terminalLogs.length, terminalVisible]);

  const statusText = loading
    ? "协调员正在生成计划..."
    : pipelineActionLoading
      ? "正在按计划编排..."
      : executing
        ? "正在启动执行会话..."
        : saving
          ? "正在保存计划..."
          : actionsLocked
            ? "任务执行中，计划与编排操作已锁定。"
            : hasPlan
              ? "计划已生成：可整包「执行」，或按工作包「按计划编排」。"
              : "等待生成协调员计划。";

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-h-[min(92vh,52rem)] w-[min(94vw,56rem)] max-w-[min(94vw,56rem)] overflow-y-auto sm:max-w-[min(94vw,56rem)]">
          <DialogHeader>
            <DialogTitle>协调员执行计划</DialogTitle>
            <DialogDescription>
              {coordinatorName
                ? `由 ${coordinatorName} 生成计划与工作包。「执行」= 整包给负责人一次会话；「按计划编排」= 串行多步骤调度。`
                : "「执行」= 整包给负责人一次会话；「按计划编排」= 串行多步骤调度。"}
              {actionsLocked ? " 当前任务进行中，保存/生成/执行/编排已禁用。" : ""}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-muted/40 px-3 py-2">
              <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
                {busy ? (
                  <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-primary" />
                ) : (
                  <Terminal className="h-3.5 w-3.5 shrink-0 text-primary" />
                )}
                <span className="truncate">{statusText}</span>
              </div>
              <button
                type="button"
                onClick={onToggleTerminal}
                className="flex shrink-0 items-center gap-1 rounded-md border border-input px-2 py-1 text-xs hover:bg-accent"
              >
                {terminalVisible ? (
                  <ChevronUp className="h-3.5 w-3.5" />
                ) : (
                  <ChevronDown className="h-3.5 w-3.5" />
                )}
                {terminalVisible ? "隐藏日志" : "显示日志"}
              </button>
            </div>

            {terminalVisible && (
              <div>
                <div className="flex items-center justify-between rounded-t border-b border-zinc-800 bg-black/80 px-2 py-1">
                  <span className="font-mono text-xs text-zinc-500">协调员终端日志</span>
                  <button
                    type="button"
                    onClick={onClearTerminal}
                    className="p-0.5 text-zinc-500 transition-colors hover:text-zinc-300"
                    title="清空日志"
                  >
                    <Eraser className="h-3 w-3" />
                  </button>
                </div>
                <ScrollArea className="h-36 overflow-hidden rounded-b bg-black">
                  <div className="space-y-0.5 p-2 font-mono text-xs">
                    {terminalLogs.length === 0 ? (
                      <div className="text-zinc-600">等待运行日志...</div>
                    ) : (
                      terminalLogs.map((line, index) => (
                        <div
                          key={`${line}-${index}`}
                          className={`whitespace-pre-wrap ${getAiLogColor(line)}`}
                        >
                          {line}
                        </div>
                      ))
                    )}
                    <div ref={terminalBottomRef} />
                  </div>
                </ScrollArea>
              </div>
            )}

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
                  readOnly={planActionsDisabled}
                  className="h-[280px]"
                  placeholder="协调员生成的计划会显示在这里，可在执行前编辑。"
                />
              </Suspense>
            )}

            {pipelineSteps.length > 0 || pipelineLoading ? (
              <TaskPipelineProgress
                steps={pipelineSteps}
                automation={pipelineAutomation}
                employees={employees}
                projectId={projectId}
                loading={pipelineLoading}
                error={pipelineError}
                notice={pipelineNotice}
                compact
                showEmployeeSelect={Boolean(onPipelineEmployeeChange)}
                actionsBusy={busy}
                actionsLocked={actionsLocked}
                onRefresh={onRefreshPipeline}
                onRetry={onRetryPipeline}
                onAbort={onAbortPipeline}
                onManualRunStep={onManualRunPipelineStep}
                onPipelineEmployeeChange={onPipelineEmployeeChange}
                onOpenStepSession={(step) => {
                  if (!step.session_id) {
                    return;
                  }
                  const stepEmployee = employees.find((item) => item.id === step.employee_id);
                  setStepLogTarget({
                    sessionRecordId: step.session_id,
                    stepTitle: step.title,
                    employeeName: stepEmployee?.name ?? null,
                  });
                }}
              />
            ) : (
              <section className="rounded-md border border-border/70 bg-muted/20 p-3">
                <p className="text-sm font-medium">协调员编排（工作包）</p>
                <p className="mt-1 text-[11px] text-muted-foreground">
                  暂无工作包。请先「重新生成计划」，或直接点「按计划编排」自动生成后启动。
                </p>
                {pipelineError && <p className="mt-1 text-xs text-destructive">{pipelineError}</p>}
              </section>
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
                className="rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
              >
                取消
              </button>
              <button
                type="button"
                onClick={onSave}
                disabled={planActionsDisabled || !hasPlan}
                title={actionsLocked ? "任务进行中，无法保存计划" : undefined}
                className="flex items-center gap-1 rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
              >
                {saving ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Save className="h-3.5 w-3.5" />
                )}
                保存计划
              </button>
              <button
                type="button"
                onClick={onRegenerate}
                disabled={planActionsDisabled}
                title={actionsLocked ? "任务进行中，无法重新生成计划" : undefined}
                className="flex items-center gap-1 rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
              >
                {loading ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="h-3.5 w-3.5" />
                )}
                重新生成计划
              </button>
              {onStartPipeline && (
                <button
                  type="button"
                  onClick={onStartPipeline}
                  disabled={planActionsDisabled || !canStartPipeline}
                  title={actionsLocked ? "任务进行中，无法启动编排" : undefined}
                  className="flex items-center gap-1 rounded-md border border-primary/40 bg-primary/10 px-3 py-1.5 text-sm text-primary hover:bg-primary/15 disabled:opacity-50"
                >
                  {pipelineActionLoading ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <GitBranch className="h-3.5 w-3.5" />
                  )}
                  按计划编排
                </button>
              )}
              <button
                type="button"
                onClick={onExecute}
                disabled={planActionsDisabled || !hasPlan || !canExecute}
                title={actionsLocked ? "任务进行中，无法执行" : undefined}
                className="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                {executing ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Play className="h-3.5 w-3.5" />
                )}
                执行
              </button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <SessionLogDialog
        open={stepLogTarget !== null}
        session={
          stepLogTarget
            ? {
                sessionRecordId: stepLogTarget.sessionRecordId,
                sessionId: stepLogTarget.sessionRecordId,
                displayName: `步骤：${stepLogTarget.stepTitle}`,
                employeeId: null,
                employeeName: stepLogTarget.employeeName,
                taskId: null,
                taskTitle: taskTitle ?? null,
                sessionKind: "execution",
              }
            : null
        }
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setStepLogTarget(null);
          }
        }}
      />
    </>
  );
}
