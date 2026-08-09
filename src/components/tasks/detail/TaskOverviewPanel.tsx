import { lazy, Suspense } from "react";
import {
  AlertTriangle,
  Bot,
  ClipboardCheck,
  ClipboardList,
  FileText,
  Network,
  Pencil,
  Save,
  X,
} from "lucide-react";

import type { Employee, Task, TaskAutomationState, TaskPipelineStep } from "@/lib/types";
import {
  formatDate,
  getAcceptanceStatusClassName,
  getAcceptanceStatusLabel,
  getTaskAutomationStatusLabel,
  type TaskAutomationDisplayState,
} from "@/lib/utils";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { DetailSection, DetailStat } from "./DetailSection";
import { TaskMcpBindingSection } from "./TaskMcpBindingSection";
import { TaskPipelineProgress } from "./TaskPipelineProgress";

const MonacoMarkdownEditor = lazy(() =>
  import("./MonacoMarkdownEditor").then((module) => ({
    default: module.MonacoMarkdownEditor,
  })),
);

interface TaskOverviewPanelProps {
  task: Task;
  description: string;
  status: string;
  coordinatorId: string;
  coordinatorName?: string;
  blockedReason: string;
  planContent: string;
  planContentDraft: string;
  planEditing: boolean;
  planSaving: boolean;
  planHasChanges: boolean;
  employees: Employee[];
  coordinatorCandidates: Employee[];
  saveError: string | null;
  automationDisplay: TaskAutomationDisplayState;
  canGenerateTesterAcceptance?: boolean;
  testerAcceptanceLoading?: boolean;
  testerAcceptanceError?: string | null;
  testerAcceptanceNotice?: string | null;
  pipelineSteps?: TaskPipelineStep[];
  pipelineAutomation?: Pick<
    TaskAutomationState,
    "pipeline_active" | "pipeline_step_index" | "phase"
  > | null;
  pipelineLoading?: boolean;
  pipelineError?: string | null;
  onRefreshPipeline?: () => void;
  onOpenPipelineStepSession?: (step: TaskPipelineStep) => void;
  acceptanceChecklist?: string;
  lastAcceptanceStatus?: string | null;
  lastAcceptanceSummary?: string | null;
  acceptanceRunning?: boolean;
  onAcceptanceChecklistChange?: (value: string) => void;
  onAcceptanceChecklistBlur?: () => void;
  onRunAcceptance?: () => void;
  onDescriptionChange: (value: string) => void;
  onDescriptionBlur: () => void;
  onBlockedReasonChange: (value: string) => void;
  onBlockedReasonBlur: () => void;
  onOpenCoordinatorPlan?: () => void;
  onGenerateTesterAcceptance?: () => void;
  onPlanEditStart: () => void;
  onPlanEditCancel: () => void;
  onPlanDraftChange: (value: string) => void;
  onPlanSave: () => void;
}

function MonacoEditorFallback({ className }: { className: string }) {
  return (
    <div
      className={`${className} flex items-center justify-center rounded-md border border-dashed text-xs text-muted-foreground`}
    >
      正在加载编辑器...
    </div>
  );
}

export function TaskOverviewPanel({
  task,
  description,
  status,
  coordinatorId,
  coordinatorName,
  blockedReason,
  planContent,
  planContentDraft,
  planEditing,
  planSaving,
  planHasChanges,
  employees,
  coordinatorCandidates,
  saveError,
  automationDisplay,
  canGenerateTesterAcceptance = false,
  testerAcceptanceLoading = false,
  testerAcceptanceError = null,
  testerAcceptanceNotice = null,
  pipelineSteps = [],
  pipelineAutomation = null,
  pipelineLoading = false,
  pipelineError = null,
  onRefreshPipeline,
  onOpenPipelineStepSession,
  acceptanceChecklist = "",
  lastAcceptanceStatus = null,
  lastAcceptanceSummary = null,
  acceptanceRunning = false,
  onAcceptanceChecklistChange,
  onAcceptanceChecklistBlur,
  onRunAcceptance,
  onDescriptionChange,
  onDescriptionBlur,
  onBlockedReasonChange,
  onBlockedReasonBlur,
  onOpenCoordinatorPlan,
  onGenerateTesterAcceptance,
  onPlanEditStart,
  onPlanEditCancel,
  onPlanDraftChange,
  onPlanSave,
}: TaskOverviewPanelProps) {
  return (
    <div className="space-y-4">
      {saveError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {saveError}
        </div>
      )}

      <DetailSection icon={FileText} title="描述" contentClassName="mt-2">
        <Suspense fallback={<MonacoEditorFallback className="h-[260px]" />}>
          <MonacoMarkdownEditor
            value={description}
            onChange={onDescriptionChange}
            onBlur={onDescriptionBlur}
            className="h-[260px]"
            placeholder="添加任务描述..."
          />
        </Suspense>
      </DetailSection>

      <DetailSection
        icon={ClipboardList}
        title="计划内容"
        contentClassName="mt-2"
        actions={
          planEditing ? (
            <>
              {planHasChanges && (
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={onPlanSave}
                  disabled={planSaving}
                  title="保存计划"
                >
                  <Save />
                  保存
                </Button>
              )}
              <Button
                type="button"
                variant="outline"
                size="xs"
                onClick={onPlanEditCancel}
                disabled={planSaving}
                title="取消编辑"
              >
                <X />
                取消
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={onPlanEditStart}
              title="编辑计划"
            >
              <Pencil />
              编辑
            </Button>
          )
        }
      >
        {planEditing ? (
          <Suspense fallback={<MonacoEditorFallback className="h-72" />}>
            <MonacoMarkdownEditor
              value={planContentDraft}
              onChange={onPlanDraftChange}
              readOnly={planSaving}
              className="h-72"
              placeholder="输入任务计划内容..."
            />
          </Suspense>
        ) : (
          <Suspense fallback={<MonacoEditorFallback className="h-72" />}>
            <MonacoMarkdownEditor
              value={planContent}
              readOnly
              className="h-72 bg-muted/30"
              placeholder="暂无计划内容"
            />
          </Suspense>
        )}
      </DetailSection>

      {status === "blocked" && (
        <div className="space-y-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-xs text-amber-900 dark:text-amber-100">
          <div className="flex items-start gap-2">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              任务已阻塞。建议指定协调员并打开「协调员计划」，拆解阻塞点后再交给执行员工推进。
              {!coordinatorId && coordinatorCandidates.length > 0
                ? " 当前项目已有协调员可选。"
                : !coordinatorId
                  ? " 当前尚未指定协调员。"
                  : ` 当前协调员：${coordinatorName ?? "已指定"}。`}
            </span>
          </div>
          <div>
            <label className="text-[11px] font-medium opacity-80">阻塞原因 *</label>
            <Textarea
              value={blockedReason}
              onChange={(e) => onBlockedReasonChange(e.target.value)}
              onBlur={onBlockedReasonBlur}
              placeholder="说明阻塞原因…"
              className="mt-1 min-h-[64px] resize-y border-amber-500/30 bg-background/80 text-foreground"
            />
          </div>
        </div>
      )}

      {(pipelineSteps.length > 0 || pipelineLoading) && (
        <TaskPipelineProgress
          steps={pipelineSteps}
          automation={pipelineAutomation}
          employees={employees}
          loading={pipelineLoading}
          error={pipelineError}
          onRefresh={onRefreshPipeline}
          onOpenStepSession={onOpenPipelineStepSession}
        />
      )}

      {(coordinatorId || pipelineSteps.length > 0) && onOpenCoordinatorPlan && (
        <DetailSection
          icon={Network}
          title="协调员计划"
          description={
            <>
              {coordinatorName
                ? `由 ${coordinatorName} 生成执行计划，确认后交给指派员工执行。`
                : "可生成或查看协调员执行计划与编排操作。"}
              {planContent.trim() ? " 任务中已保存一份计划内容。" : " 当前还没有保存的协调员计划。"}
              {pipelineSteps.length > 0 ? " 完整重试/转人工/改执行人请在编排面板中操作。" : ""}
            </>
          }
          actions={
            <Button type="button" variant="outline" size="sm" onClick={onOpenCoordinatorPlan}>
              <Network />
              {pipelineSteps.length > 0
                ? "打开编排面板"
                : planContent.trim()
                  ? "查看协调员计划"
                  : "生成协调员计划"}
            </Button>
          }
        />
      )}

      <DetailSection
        icon={ClipboardCheck}
        title={
          <>
            测试验收
            <span
              className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${getAcceptanceStatusClassName(lastAcceptanceStatus)}`}
            >
              {getAcceptanceStatusLabel(lastAcceptanceStatus)}
            </span>
          </>
        }
        description="配置项目测试命令后可客观验收；命令失败为硬失败。也可生成/编辑验收清单。"
        actions={
          <>
            {canGenerateTesterAcceptance && onGenerateTesterAcceptance && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={onGenerateTesterAcceptance}
                disabled={testerAcceptanceLoading || acceptanceRunning}
              >
                <ClipboardCheck />
                {testerAcceptanceLoading ? "生成中…" : "生成验收清单"}
              </Button>
            )}
            {onRunAcceptance && (
              <Button
                type="button"
                size="sm"
                onClick={onRunAcceptance}
                disabled={acceptanceRunning || testerAcceptanceLoading}
                className="border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
              >
                {acceptanceRunning ? "验收中…" : "运行验收"}
              </Button>
            )}
          </>
        }
      >
        <div className="space-y-2">
          {lastAcceptanceSummary && (
            <div className="rounded-md border border-border/60 bg-background/70 px-3 py-2 text-xs text-muted-foreground">
              最近结果：{lastAcceptanceSummary}
            </div>
          )}
          {onAcceptanceChecklistChange && (
            <div className="space-y-1">
              <label className="text-[11px] text-muted-foreground">验收清单</label>
              <Suspense fallback={<MonacoEditorFallback className="h-40" />}>
                <MonacoMarkdownEditor
                  value={acceptanceChecklist}
                  onChange={onAcceptanceChecklistChange}
                  onBlur={onAcceptanceChecklistBlur}
                  className="h-40 bg-background"
                  placeholder="可手写或由测试员 AI 生成验收清单…"
                />
              </Suspense>
            </div>
          )}
          {testerAcceptanceError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {testerAcceptanceError}
            </div>
          )}
          {testerAcceptanceNotice && !testerAcceptanceError && (
            <div className="rounded-md border border-border/60 bg-background/70 px-3 py-2 text-xs text-muted-foreground">
              {testerAcceptanceNotice}
            </div>
          )}
        </div>
      </DetailSection>

      <DetailSection
        icon={Bot}
        title="自动质控"
        description="原任务内的自动审核与修复闭环状态；开关入口在任务卡片右键菜单。"
        actions={
          <span
            className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium ${
              automationDisplay.enabled
                ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
                : "bg-muted text-muted-foreground"
            }`}
          >
            {automationDisplay.enabled ? "已开启" : "未开启"}
          </span>
        }
      >
        {automationDisplay.enabled ? (
          <div className="space-y-2">
            <div className="grid gap-2 sm:grid-cols-2">
              <DetailStat
                label="闭环阶段"
                value={getTaskAutomationStatusLabel(automationDisplay.status)}
              />
              <DetailStat label="自动修复轮次" value={automationDisplay.roundCount ?? 0} />
              <DetailStat
                label="最近更新时间"
                value={
                  automationDisplay.updatedAt ? formatDate(automationDisplay.updatedAt) : "暂无"
                }
              />
              <DetailStat
                label="状态来源"
                value={automationDisplay.source === "automation_state" ? "自动化状态" : "任务配置"}
              />
            </div>
            <div className="rounded-md border border-dashed border-border/60 bg-background/70 px-3 py-2 text-xs text-muted-foreground">
              自动质控不会替代现有“审核结果 →
              修复”手动路径。手动修复仍然通过创建新任务推进；自动质控接线后则在原任务内完成审核与修复闭环。
            </div>
            {automationDisplay.note && (
              <div className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-200">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                <span>{automationDisplay.note}</span>
              </div>
            )}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            未开启自动质控。可在任务卡片右键菜单开启；手动「审核结果 → 修复」路径不受影响。
          </p>
        )}
      </DetailSection>

      <TaskMcpBindingSection taskId={task.id} />
    </div>
  );
}
