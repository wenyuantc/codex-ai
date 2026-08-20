import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";

import type { Employee, Task, TaskStatus } from "@/lib/types";
import { ACTIVE_TASK_STATUSES, PRIORITIES } from "@/lib/types";
import {
  formatDate,
  formatDuration,
  getPriorityLabel,
  getStatusLabel,
  getTaskElapsedSeconds,
  shouldShowTaskTimer,
} from "@/lib/utils";
import { useSharedNow } from "@/hooks/useSharedNow";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TaskDeliverySection } from "./TaskDeliverySection";

const UNASSIGNED_VALUE = "__unassigned__";

function employeeProviderLabel(provider: string | null | undefined): string {
  switch (provider) {
    case "claude":
      return "Claude";
    case "opencode":
      return "OpenCode";
    case "grok":
      return "Grok";
    default:
      return "Codex";
  }
}

function employeeDisplayName(employees: Employee[], id: string): string {
  const emp = employees.find((item) => item.id === id);
  return emp ? `${emp.name} · ${employeeProviderLabel(emp.ai_provider)}` : "";
}

function SidebarField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-1">
      <p className="text-[11px] font-medium text-muted-foreground">{label}</p>
      {children}
    </div>
  );
}

function SidebarGroup({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className="space-y-3">
      <p className="text-xs font-medium text-muted-foreground">{label}</p>
      {children}
    </section>
  );
}

interface TaskPropertiesSidebarProps {
  task: Task;
  projectTasks: Task[];
  status: string;
  priority: string;
  assigneeId: string;
  reviewerId: string;
  coordinatorId: string;
  dueDate: string;
  milestoneId: string;
  timeStartedAt: string | null;
  timeSpentSeconds: number;
  completedAt: string | null;
  employees: Employee[];
  reviewerCandidates: Employee[];
  coordinatorCandidates: Employee[];
  isRunning: boolean;
  deletingTask: boolean;
  onStatusChange: (value: TaskStatus) => void;
  onPriorityChange: (value: string) => void;
  onAssigneeChange: (value: string) => void;
  onReviewerChange: (value: string) => void;
  onCoordinatorChange: (value: string) => void;
  onDueDateChange: (value: string) => void;
  onDueDateBlur: () => void;
  onMilestoneChange: (value: string) => void;
  onDeliveryError?: (message: string) => void;
  onDeleteRequest: () => void;
}

export function TaskPropertiesSidebar({
  task,
  projectTasks,
  status,
  priority,
  assigneeId,
  reviewerId,
  coordinatorId,
  dueDate,
  milestoneId,
  timeStartedAt,
  timeSpentSeconds,
  completedAt,
  employees,
  reviewerCandidates,
  coordinatorCandidates,
  isRunning,
  deletingTask,
  onStatusChange,
  onPriorityChange,
  onAssigneeChange,
  onReviewerChange,
  onCoordinatorChange,
  onDueDateChange,
  onDueDateBlur,
  onMilestoneChange,
  onDeliveryError,
  onDeleteRequest,
}: TaskPropertiesSidebarProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const timerNow = useSharedNow(Boolean(timeStartedAt));
  const elapsedSeconds = getTaskElapsedSeconds(
    {
      time_started_at: timeStartedAt,
      time_spent_seconds: timeSpentSeconds,
    },
    timerNow,
  );
  const timerStatus = timeStartedAt
    ? t("detail.sidebar.timerRunning")
    : completedAt
      ? t("detail.sidebar.timerCompleted")
      : timeSpentSeconds > 0
        ? t("detail.sidebar.timerPaused")
        : t("detail.sidebar.timerNotStarted");
  const showTimer = shouldShowTaskTimer({
    time_started_at: timeStartedAt,
    time_spent_seconds: timeSpentSeconds,
    completed_at: completedAt,
  });

  return (
    <div className="flex flex-col gap-4 p-4">
      <SidebarGroup label={t("detail.sidebar.properties")}>
        <SidebarField label={t("detail.sidebar.status")}>
          <Select
            value={status}
            onValueChange={(value) => value && onStatusChange(value as TaskStatus)}
          >
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? getStatusLabel(value)
                    : t("detail.sidebar.selectStatus")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {/* Archived: keep current item so the controlled value stays valid. */}
              {status === "archived" && (
                <SelectItem value="archived" disabled>
                  {getStatusLabel("archived")}
                </SelectItem>
              )}
              {ACTIVE_TASK_STATUSES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {getStatusLabel(item.value)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label={t("detail.sidebar.priority")}>
          <Select value={priority} onValueChange={(value) => value && onPriorityChange(value)}>
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? getPriorityLabel(value)
                    : t("detail.sidebar.selectPriority")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {PRIORITIES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {getPriorityLabel(item.value)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label={t("detail.sidebar.assignee")}>
          <Select
            value={assigneeId || UNASSIGNED_VALUE}
            onValueChange={(value) =>
              onAssigneeChange(!value || value === UNASSIGNED_VALUE ? "" : value)
            }
          >
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  !value || value === UNASSIGNED_VALUE
                    ? t("unassigned")
                    : employeeDisplayName(employees, value) || t("unassigned")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>{t("unassigned")}</SelectItem>
              {employees.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {employeeProviderLabel(emp.ai_provider)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label={t("detail.sidebar.reviewer")}>
          <Select
            value={reviewerId || UNASSIGNED_VALUE}
            onValueChange={(value) =>
              onReviewerChange(!value || value === UNASSIGNED_VALUE ? "" : value)
            }
          >
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  !value || value === UNASSIGNED_VALUE
                    ? t("detail.sidebar.unspecified")
                    : employeeDisplayName(employees, value) || t("detail.sidebar.unspecified")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>{t("detail.sidebar.unspecified")}</SelectItem>
              {reviewerCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {employeeProviderLabel(emp.ai_provider)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label={t("detail.sidebar.coordinator")}>
          <Select
            value={coordinatorId || UNASSIGNED_VALUE}
            onValueChange={(value) =>
              onCoordinatorChange(!value || value === UNASSIGNED_VALUE ? "" : value)
            }
          >
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  !value || value === UNASSIGNED_VALUE
                    ? t("detail.sidebar.unspecified")
                    : employeeDisplayName(coordinatorCandidates, value) ||
                      t("detail.sidebar.unspecified")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>{t("detail.sidebar.unspecified")}</SelectItem>
              {coordinatorCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {employeeProviderLabel(emp.ai_provider)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>
      </SidebarGroup>

      <div className="border-t border-border/60" />

      <SidebarGroup label={t("detail.sidebar.delivery")}>
        <TaskDeliverySection
          bare
          task={task}
          projectTasks={projectTasks}
          dueDate={dueDate}
          milestoneId={milestoneId}
          onDueDateChange={onDueDateChange}
          onDueDateBlur={onDueDateBlur}
          onMilestoneChange={onMilestoneChange}
          onError={onDeliveryError}
        />
      </SidebarGroup>

      {showTimer ? (
        <>
          <div className="border-t border-border/60" />

          <SidebarGroup label={t("detail.sidebar.elapsed")}>
            <div className="space-y-1.5 text-xs">
              <div className="flex items-center justify-between gap-2">
                <span className="text-muted-foreground">{t("detail.sidebar.totalElapsed")}</span>
                <span className="font-medium text-foreground">
                  {formatDuration(elapsedSeconds)}
                </span>
              </div>
              <div className="flex items-center justify-between gap-2">
                <span className="text-muted-foreground">{t("detail.sidebar.timerStatus")}</span>
                <span className="font-medium text-foreground">{timerStatus}</span>
              </div>
              <div className="flex items-center justify-between gap-2">
                <span className="text-muted-foreground">{t("detail.sidebar.timerStartedAt")}</span>
                <span className="font-medium text-foreground">
                  {timeStartedAt ? formatDate(timeStartedAt) : t("detail.sidebar.timerNotStarted")}
                </span>
              </div>
              <div className="flex items-center justify-between gap-2">
                <span className="text-muted-foreground">{t("detail.sidebar.completedAt")}</span>
                <span className="font-medium text-foreground">
                  {completedAt ? formatDate(completedAt) : t("detail.sidebar.notCompleted")}
                </span>
              </div>
            </div>
          </SidebarGroup>

          <div className="border-t border-border/60" />
        </>
      ) : (
        <div className="border-t border-border/60" />
      )}

      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={onDeleteRequest}
        disabled={isRunning || deletingTask}
        title={
          isRunning ? t("detail.sidebar.deleteDisabledRunning") : t("detail.sidebar.deleteTask")
        }
        className="w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
      >
        <Trash2 />
        {t("detail.sidebar.deleteTask")}
      </Button>
    </div>
  );
}
