import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
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
import { mapAutomationNote } from "@/lib/i18n/mapAutomationNote";
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
  coordinatorPlanGenerating?: boolean;
  onGenerateTesterAcceptance?: () => void;
  onPlanEditStart: () => void;
  onPlanEditCancel: () => void;
  onPlanDraftChange: (value: string) => void;
  onPlanSave: () => void;
}

function MonacoEditorFallback({ className }: { className: string }) {
  const { t } = useTranslation("tasks");
  return (
    <div
      className={`${className} flex items-center justify-center rounded-md border border-dashed text-xs text-muted-foreground`}
    >
      {t("detail.overview.loadingEditor")}
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
  coordinatorPlanGenerating = false,
  onGenerateTesterAcceptance,
  onPlanEditStart,
  onPlanEditCancel,
  onPlanDraftChange,
  onPlanSave,
}: TaskOverviewPanelProps) {
  const { t } = useTranslation(["tasks", "common"]);
  return (
    <div className="space-y-4">
      {saveError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {saveError}
        </div>
      )}

      <DetailSection
        icon={FileText}
        title={t("detail.overview.description")}
        contentClassName="mt-2"
      >
        <Suspense fallback={<MonacoEditorFallback className="h-[260px]" />}>
          <MonacoMarkdownEditor
            value={description}
            onChange={onDescriptionChange}
            onBlur={onDescriptionBlur}
            className="h-[260px]"
            placeholder={t("detail.overview.descriptionPlaceholder")}
          />
        </Suspense>
      </DetailSection>

      <DetailSection
        icon={ClipboardList}
        title={t("detail.overview.planContent")}
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
                  title={t("detail.overview.savePlanTitle")}
                >
                  <Save />
                  {t("common:save")}
                </Button>
              )}
              <Button
                type="button"
                variant="outline"
                size="xs"
                onClick={onPlanEditCancel}
                disabled={planSaving}
                title={t("detail.overview.cancelEditTitle")}
              >
                <X />
                {t("common:cancel")}
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={onPlanEditStart}
              title={t("detail.overview.editPlanTitle")}
            >
              <Pencil />
              {t("common:edit")}
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
              placeholder={t("detail.overview.planDraftPlaceholder")}
            />
          </Suspense>
        ) : (
          <Suspense fallback={<MonacoEditorFallback className="h-72" />}>
            <MonacoMarkdownEditor
              value={planContent}
              readOnly
              className="h-72 bg-muted/30"
              placeholder={t("detail.overview.noPlanYet")}
            />
          </Suspense>
        )}
      </DetailSection>

      {status === "blocked" && (
        <div className="space-y-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-xs text-amber-900 dark:text-amber-100">
          <div className="flex items-start gap-2">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              {t("detail.overview.blockedBanner")}
              {!coordinatorId && coordinatorCandidates.length > 0
                ? ` ${t("detail.overview.blockedHasCoordinators")}`
                : !coordinatorId
                  ? ` ${t("detail.overview.blockedNoCoordinator")}`
                  : ` ${t("detail.overview.blockedCurrentCoordinator", {
                      name: coordinatorName ?? t("detail.overview.blockedCoordinatorAssigned"),
                    })}`}
            </span>
          </div>
          <div>
            <label className="text-[11px] font-medium opacity-80">
              {t("detail.overview.blockedReasonLabel")}
            </label>
            <Textarea
              value={blockedReason}
              onChange={(e) => onBlockedReasonChange(e.target.value)}
              onBlur={onBlockedReasonBlur}
              placeholder={t("detail.overview.blockedReasonPlaceholder")}
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
          title={t("detail.overview.coordinatorPlan")}
          description={
            <>
              {coordinatorName
                ? t("detail.overview.coordinatorPlanDescWithName", { name: coordinatorName })
                : t("detail.overview.coordinatorPlanDescGeneric")}
              {planContent.trim()
                ? ` ${t("detail.overview.planSavedNote")}`
                : ` ${t("detail.overview.planNotSavedNote")}`}
              {pipelineSteps.length > 0 ? ` ${t("detail.overview.orchestrationNote")}` : ""}
            </>
          }
          actions={
            <Button type="button" variant="outline" size="sm" onClick={onOpenCoordinatorPlan}>
              <Network />
              {coordinatorPlanGenerating
                ? t("detail.overview.viewCoordinatorPlanProgress")
                : pipelineSteps.length > 0
                  ? t("detail.overview.openOrchestration")
                  : planContent.trim()
                    ? t("detail.overview.viewCoordinatorPlan")
                    : t("detail.overview.openCoordinatorPlan")}
            </Button>
          }
        />
      )}

      <DetailSection
        icon={ClipboardCheck}
        title={
          <>
            {t("detail.overview.testerAcceptance")}
            <span
              className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${getAcceptanceStatusClassName(lastAcceptanceStatus)}`}
            >
              {getAcceptanceStatusLabel(lastAcceptanceStatus)}
            </span>
          </>
        }
        description={t("detail.overview.testerAcceptanceDesc")}
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
                {testerAcceptanceLoading
                  ? t("detail.overview.generatingChecklist")
                  : t("detail.overview.generateChecklist")}
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
                {acceptanceRunning
                  ? t("detail.overview.accepting")
                  : t("detail.overview.runAcceptance")}
              </Button>
            )}
          </>
        }
      >
        <div className="space-y-2">
          {lastAcceptanceSummary && (
            <div className="rounded-md border border-border/60 bg-background/70 px-3 py-2 text-xs text-muted-foreground">
              {t("detail.overview.latestResult", { summary: lastAcceptanceSummary })}
            </div>
          )}
          {onAcceptanceChecklistChange && (
            <div className="space-y-1">
              <label className="text-[11px] text-muted-foreground">
                {t("detail.overview.acceptanceChecklist")}
              </label>
              <Suspense fallback={<MonacoEditorFallback className="h-40" />}>
                <MonacoMarkdownEditor
                  value={acceptanceChecklist}
                  onChange={onAcceptanceChecklistChange}
                  onBlur={onAcceptanceChecklistBlur}
                  className="h-40 bg-background"
                  placeholder={t("detail.overview.checklistPlaceholder")}
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
        title={t("detail.overview.autoQc")}
        description={t("detail.overview.autoQcDesc")}
        actions={
          <span
            className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium ${
              automationDisplay.enabled
                ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
                : "bg-muted text-muted-foreground"
            }`}
          >
            {automationDisplay.enabled
              ? t("common:automation.enabled")
              : t("common:automation.disabled")}
          </span>
        }
      >
        {automationDisplay.enabled ? (
          <div className="space-y-2">
            <div className="grid gap-2 sm:grid-cols-2">
              <DetailStat
                label={t("detail.overview.loopPhase")}
                value={getTaskAutomationStatusLabel(automationDisplay.status)}
              />
              <DetailStat
                label={t("detail.overview.autoFixRounds")}
                value={automationDisplay.roundCount ?? 0}
              />
              <DetailStat
                label={t("detail.overview.lastUpdated")}
                value={
                  automationDisplay.updatedAt
                    ? formatDate(automationDisplay.updatedAt)
                    : t("detail.overview.none")
                }
              />
              <DetailStat
                label={t("detail.overview.statusSource")}
                value={
                  automationDisplay.source === "automation_state"
                    ? t("detail.overview.sourceAutomationState")
                    : t("detail.overview.sourceTaskConfig")
                }
              />
            </div>
            <div className="rounded-md border border-dashed border-border/60 bg-background/70 px-3 py-2 text-xs text-muted-foreground">
              {t("detail.overview.autoQcDisclaimer")}
            </div>
            {automationDisplay.note && (
              <div className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-200">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                <span>{mapAutomationNote(automationDisplay.note)}</span>
              </div>
            )}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">{t("detail.overview.autoQcOffHint")}</p>
        )}
      </DetailSection>

      <TaskMcpBindingSection
        taskId={task.id}
        assignee={employees.find((item) => item.id === task.assignee_id) ?? null}
      />
    </div>
  );
}
