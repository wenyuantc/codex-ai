import type { ReactNode } from "react";
import { Trash2 } from "lucide-react";

import type { Employee, Task, TaskStatus } from "@/lib/types";
import { ACTIVE_TASK_STATUSES, PRIORITIES, TASK_STATUSES } from "@/lib/types";
import { formatDate, formatDuration, getTaskElapsedSeconds } from "@/lib/utils";
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
  const timerNow = useSharedNow(Boolean(timeStartedAt));
  const elapsedSeconds = getTaskElapsedSeconds(
    {
      time_started_at: timeStartedAt,
      time_spent_seconds: timeSpentSeconds,
    },
    timerNow,
  );
  const timerStatus = timeStartedAt
    ? "计时中"
    : completedAt
      ? "已完成"
      : timeSpentSeconds > 0
        ? "待继续"
        : "未开始";

  return (
    <div className="flex flex-col gap-4 p-4">
      <SidebarGroup label="属性">
        <SidebarField label="状态">
          <Select
            value={status}
            onValueChange={(value) => value && onStatusChange(value as TaskStatus)}
          >
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? (TASK_STATUSES.find((item) => item.value === value)?.label ?? value)
                    : "选择状态"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {/* Archived: keep current item so the controlled value stays valid. */}
              {status === "archived" && (
                <SelectItem value="archived" disabled>
                  已归档
                </SelectItem>
              )}
              {ACTIVE_TASK_STATUSES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label="优先级">
          <Select value={priority} onValueChange={(value) => value && onPriorityChange(value)}>
            <SelectTrigger className="h-8 w-full rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? (PRIORITIES.find((item) => item.value === value)?.label ?? value)
                    : "选择优先级"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {PRIORITIES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label="指派人">
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
                    ? "未指派"
                    : employeeDisplayName(employees, value) || "未指派"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
              {employees.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {employeeProviderLabel(emp.ai_provider)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label="审查员">
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
                    ? "未指定"
                    : employeeDisplayName(employees, value) || "未指定"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
              {reviewerCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {employeeProviderLabel(emp.ai_provider)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SidebarField>

        <SidebarField label="协调员">
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
                    ? "未指定"
                    : employeeDisplayName(coordinatorCandidates, value) || "未指定"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
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

      <SidebarGroup label="交付">
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

      <div className="border-t border-border/60" />

      <SidebarGroup label="耗时">
        <div className="space-y-1.5 text-xs">
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">累计耗时</span>
            <span className="font-medium text-foreground">{formatDuration(elapsedSeconds)}</span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">计时状态</span>
            <span className="font-medium text-foreground">{timerStatus}</span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">计时开始</span>
            <span className="font-medium text-foreground">
              {timeStartedAt ? formatDate(timeStartedAt) : "未开始"}
            </span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">完成时间</span>
            <span className="font-medium text-foreground">
              {completedAt ? formatDate(completedAt) : "未完成"}
            </span>
          </div>
        </div>
      </SidebarGroup>

      <div className="border-t border-border/60" />

      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={onDeleteRequest}
        disabled={isRunning || deletingTask}
        title={isRunning ? "运行中的任务不能删除，请先停止" : "删除任务"}
        className="w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
      >
        <Trash2 />
        删除任务
      </Button>
    </div>
  );
}
