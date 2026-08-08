import { GitBranch, Loader2, Play, RefreshCw, ScrollText } from "lucide-react";

import type { Employee, TaskAutomationState, TaskPipelineStep } from "@/lib/types";
import { formatDate } from "@/lib/utils";
import {
  getPipelineProgressSummary,
  isPipelineStepManuallyRunnable,
  pipelineStatusLabel,
  pipelineStepStatusDotClass,
  pipelineStepStatusTextClass,
  shouldShowPipelineManualRun,
  shouldShowPipelineRetry,
} from "@/lib/pipelineUi";

export interface TaskPipelineProgressProps {
  steps: TaskPipelineStep[];
  automation?: Pick<
    TaskAutomationState,
    "pipeline_active" | "pipeline_step_index" | "phase"
  > | null;
  employees?: Employee[];
  loading?: boolean;
  error?: string | null;
  notice?: string | null;
  compact?: boolean;
  /** When true, show per-step employee select (dialog mode). */
  showEmployeeSelect?: boolean;
  projectId?: string;
  actionsBusy?: boolean;
  actionsLocked?: boolean;
  onRefresh?: () => void;
  onRetry?: () => void;
  onAbort?: () => void;
  onManualRunStep?: (step: TaskPipelineStep) => void;
  onOpenStepSession?: (step: TaskPipelineStep) => void;
  onPipelineEmployeeChange?: (stepId: string, employeeId: string) => void;
}

export function TaskPipelineProgress({
  steps,
  automation = null,
  employees = [],
  loading = false,
  error = null,
  notice = null,
  compact = false,
  showEmployeeSelect = false,
  projectId,
  actionsBusy = false,
  actionsLocked = false,
  onRefresh,
  onRetry,
  onAbort,
  onManualRunStep,
  onOpenStepSession,
  onPipelineEmployeeChange,
}: TaskPipelineProgressProps) {
  if (steps.length === 0 && !loading) {
    return null;
  }

  const summary = getPipelineProgressSummary(steps, automation);
  const showRetry = Boolean(onRetry) && shouldShowPipelineRetry(steps, automation);
  const showManualRun = Boolean(onManualRunStep) && shouldShowPipelineManualRun(steps, automation);
  const projectEmployees = employees.filter(
    (employee) => !employee.project_id || !projectId || employee.project_id === projectId,
  );
  const listMaxClass = compact ? "max-h-48" : "max-h-64";

  return (
    <section className="space-y-2 rounded-md border border-border/70 bg-muted/20 p-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="space-y-0.5">
          <p className="flex items-center gap-1.5 text-sm font-medium text-foreground">
            <GitBranch className="h-3.5 w-3.5 text-primary" />
            协调员编排
          </p>
          <p className="text-[11px] text-muted-foreground">
            {summary ?? (loading ? "加载工作包..." : "串行工作包进度")}
          </p>
        </div>
        <div className="flex flex-wrap gap-1.5">
          {showRetry && (
            <button
              type="button"
              className="rounded-md border border-input px-2 py-1 text-xs hover:bg-accent disabled:opacity-50"
              disabled={actionsBusy || actionsLocked}
              onClick={onRetry}
              title={actionsLocked ? "任务执行中，请等待当前步骤结束" : "重试失败步骤"}
            >
              重试失败步骤
            </button>
          )}
          {onAbort && (
            <button
              type="button"
              className="rounded-md border border-input px-2 py-1 text-xs hover:bg-accent disabled:opacity-50"
              disabled={actionsBusy}
              onClick={onAbort}
            >
              转人工
            </button>
          )}
          {onRefresh && (
            <button
              type="button"
              className="inline-flex items-center gap-1 rounded-md border border-input px-2 py-1 text-xs hover:bg-accent disabled:opacity-50"
              disabled={loading || actionsBusy}
              onClick={onRefresh}
            >
              {loading ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <RefreshCw className="h-3 w-3" />
              )}
              刷新
            </button>
          )}
        </div>
      </div>

      {notice && <p className="text-xs text-emerald-700 dark:text-emerald-300">{notice}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
      {showManualRun && automation?.phase === "manual_control" && (
        <p className="text-[11px] text-amber-800 dark:text-amber-200">
          已转人工：可对未完成步骤点击「手动运行」。
        </p>
      )}

      {loading && steps.length === 0 ? (
        <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" />
          加载工作包...
        </p>
      ) : steps.length === 0 ? null : (
        <>
          <div className="flex items-center gap-1 px-0.5" aria-hidden>
            {steps.map((step, index) => (
              <div key={step.id} className="flex min-w-0 flex-1 items-center gap-1">
                <div
                  className={`h-2.5 w-2.5 shrink-0 rounded-full ${pipelineStepStatusDotClass(step.status)}`}
                  title={`${step.step_index + 1}. ${step.title} · ${pipelineStatusLabel(step.status)}`}
                />
                {index < steps.length - 1 && (
                  <div
                    className={`h-0.5 min-w-0 flex-1 rounded ${
                      step.status === "succeeded" ? "bg-emerald-500/60" : "bg-border"
                    }`}
                  />
                )}
              </div>
            ))}
          </div>

          <div className={`${listMaxClass} space-y-2 overflow-y-auto pr-1`}>
            {steps.map((step) => {
              const stepEmployee = employees.find((item) => item.id === step.employee_id);
              const stepBusy = step.status === "running" || step.status === "launching";
              const canManualRun =
                showManualRun && isPipelineStepManuallyRunnable(step.status) && !stepBusy;
              return (
                <div
                  key={step.id}
                  className="rounded-md border border-border bg-background/70 px-3 py-2 text-xs"
                >
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="min-w-0 font-medium text-foreground">
                      {step.step_index + 1}. {step.title}
                      <span
                        className={`ml-2 font-normal ${pipelineStepStatusTextClass(step.status)}`}
                      >
                        [{pipelineStatusLabel(step.status)}]
                      </span>
                    </div>
                    <div className="flex flex-wrap items-center gap-1.5">
                      {canManualRun && onManualRunStep && (
                        <button
                          type="button"
                          className="inline-flex items-center gap-1 rounded border border-primary/40 bg-primary/10 px-2 py-1 text-[11px] text-primary hover:bg-primary/15 disabled:opacity-50"
                          disabled={actionsBusy || actionsLocked}
                          title={
                            actionsLocked ? "任务执行中，请等待当前步骤结束" : "手动运行此编排步骤"
                          }
                          onClick={() => onManualRunStep(step)}
                        >
                          <Play className="h-3 w-3" />
                          手动运行
                        </button>
                      )}
                      {onOpenStepSession && (
                        <button
                          type="button"
                          className="inline-flex items-center gap-1 rounded border border-input px-2 py-1 text-[11px] hover:bg-accent disabled:opacity-50"
                          disabled={!step.session_id}
                          title={step.session_id ? "查看该步骤执行日志" : "该步骤尚无执行会话"}
                          onClick={() => onOpenStepSession(step)}
                        >
                          <ScrollText className="h-3 w-3" />
                          执行日志
                        </button>
                      )}
                      {showEmployeeSelect && onPipelineEmployeeChange && (
                        <select
                          className="max-w-[12rem] rounded border border-border bg-background px-2 py-1"
                          value={step.employee_id ?? ""}
                          disabled={actionsBusy || stepBusy || actionsLocked}
                          onChange={(event) =>
                            onPipelineEmployeeChange(step.id, event.target.value)
                          }
                        >
                          <option value="">回落任务负责人</option>
                          {projectEmployees.map((employee) => (
                            <option key={employee.id} value={employee.id}>
                              {employee.name} ({employee.ai_provider})
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                  </div>
                  {step.goal && <p className="mt-1 text-muted-foreground">目标：{step.goal}</p>}
                  {step.success_criteria && (
                    <p className="text-muted-foreground">成功标准：{step.success_criteria}</p>
                  )}
                  {stepEmployee && (
                    <p className="text-muted-foreground">建议执行：{stepEmployee.name}</p>
                  )}
                  {step.last_error && <p className="mt-1 text-destructive">{step.last_error}</p>}
                  {step.handoff_summary && step.status === "succeeded" && (
                    <p className="mt-1 line-clamp-2 text-muted-foreground">
                      交接：{step.handoff_summary}
                    </p>
                  )}
                  {(step.started_at || step.ended_at) && (
                    <p className="mt-1 text-[11px] text-muted-foreground">
                      {step.started_at ? `开始 ${formatDate(step.started_at)}` : null}
                      {step.started_at && step.ended_at ? " · " : null}
                      {step.ended_at ? `结束 ${formatDate(step.ended_at)}` : null}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}
    </section>
  );
}
