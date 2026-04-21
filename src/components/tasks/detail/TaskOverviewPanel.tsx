import { Trash2 } from "lucide-react";

import type { Employee, TaskStatus } from "@/lib/types";
import { ACTIVE_TASK_STATUSES, PRIORITIES, TASK_STATUSES } from "@/lib/types";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

const UNASSIGNED_VALUE = "__unassigned__";

interface TaskOverviewPanelProps {
  title: string;
  description: string;
  status: string;
  priority: string;
  assigneeId: string;
  reviewerId: string;
  employees: Employee[];
  reviewerCandidates: Employee[];
  saveError: string | null;
  isRunning: boolean;
  deletingTask: boolean;
  onTitleChange: (value: string) => void;
  onTitleBlur: () => void;
  onDescriptionChange: (value: string) => void;
  onDescriptionBlur: () => void;
  onStatusChange: (value: TaskStatus) => void;
  onPriorityChange: (value: string) => void;
  onAssigneeChange: (value: string) => void;
  onReviewerChange: (value: string) => void;
  onDeleteRequest: () => void;
}

export function TaskOverviewPanel({
  title,
  description,
  status,
  priority,
  assigneeId,
  reviewerId,
  employees,
  reviewerCandidates,
  saveError,
  isRunning,
  deletingTask,
  onTitleChange,
  onTitleBlur,
  onDescriptionChange,
  onDescriptionBlur,
  onStatusChange,
  onPriorityChange,
  onAssigneeChange,
  onReviewerChange,
  onDeleteRequest,
}: TaskOverviewPanelProps) {
  return (
    <div className="space-y-4">
      <Input
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
        onBlur={onTitleBlur}
        className="text-lg font-semibold border-none px-0 focus-visible:ring-0"
        placeholder="任务标题"
      />

      {saveError && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {saveError}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <div className="flex shrink-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">状态</span>
          <Select
            value={status}
            disabled={status === "archived"}
            onValueChange={(value) => value && onStatusChange(value as TaskStatus)}
          >
            <SelectTrigger className="h-7 w-[104px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? TASK_STATUSES.find((item) => item.value === value)?.label ?? value
                    : "选择状态"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {ACTIVE_TASK_STATUSES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">优先级</span>
          <Select value={priority} onValueChange={(value) => value && onPriorityChange(value)}>
            <SelectTrigger className="h-7 w-[92px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) =>
                  typeof value === "string"
                    ? PRIORITIES.find((item) => item.value === value)?.label ?? value
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
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">指派</span>
          <Select
            value={assigneeId || UNASSIGNED_VALUE}
            onValueChange={(value) => onAssigneeChange(!value || value === UNASSIGNED_VALUE ? "" : value)}
          >
            <SelectTrigger className="h-7 w-[176px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === UNASSIGNED_VALUE) {
                    return "未指派";
                  }

                  return employees.find((emp) => emp.id === value)?.name ?? "未指派";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
              {employees.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">审查员</span>
          <Select
            value={reviewerId || UNASSIGNED_VALUE}
            onValueChange={(value) => onReviewerChange(!value || value === UNASSIGNED_VALUE ? "" : value)}
          >
            <SelectTrigger className="h-7 w-[176px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === UNASSIGNED_VALUE) {
                    return "未指定";
                  }

                  return employees.find((emp) => emp.id === value)?.name ?? "未指定";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
              {reviewerCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <button
          onClick={onDeleteRequest}
          disabled={isRunning || deletingTask}
          className="ml-auto p-1 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
          title={isRunning ? "运行中的任务不能删除，请先停止" : "删除任务"}
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>

      <div>
        <label className="text-xs font-medium text-muted-foreground">
          描述
        </label>
        <Textarea
          value={description}
          onChange={(e) => onDescriptionChange(e.target.value)}
          onBlur={onDescriptionBlur}
          className="mt-1 min-h-[220px] resize-y"
          placeholder="添加任务描述..."
        />
      </div>
    </div>
  );
}
