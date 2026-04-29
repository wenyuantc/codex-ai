import { lazy, Suspense, useEffect, useState } from "react";
import { Clock, Pencil, Save, Trash2, X } from "lucide-react";

import type { Employee, TaskStatus } from "@/lib/types";
import { ACTIVE_TASK_STATUSES, PRIORITIES, TASK_STATUSES } from "@/lib/types";
import { formatDate, formatDuration, getTaskElapsedSeconds } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const UNASSIGNED_VALUE = "__unassigned__";
const MonacoMarkdownEditor = lazy(() => import("./MonacoMarkdownEditor").then((module) => ({
  default: module.MonacoMarkdownEditor,
})));

interface TaskOverviewPanelProps {
  title: string;
  description: string;
  status: string;
  priority: string;
  assigneeId: string;
  reviewerId: string;
  coordinatorId: string;
  createdAt: string;
  timeStartedAt: string | null;
  timeSpentSeconds: number;
  completedAt: string | null;
  planContent: string;
  planContentDraft: string;
  planEditing: boolean;
  planSaving: boolean;
  planHasChanges: boolean;
  employees: Employee[];
  reviewerCandidates: Employee[];
  coordinatorCandidates: Employee[];
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
  onCoordinatorChange: (value: string) => void;
  onPlanEditStart: () => void;
  onPlanEditCancel: () => void;
  onPlanDraftChange: (value: string) => void;
  onPlanSave: () => void;
  onDeleteRequest: () => void;
}

function MonacoEditorFallback({ className }: { className: string }) {
  return (
    <div className={`${className} flex items-center justify-center rounded-md border border-dashed text-xs text-muted-foreground`}>
      正在加载编辑器...
    </div>
  );
}

export function TaskOverviewPanel({
  title,
  description,
  status,
  priority,
  assigneeId,
  reviewerId,
  coordinatorId,
  createdAt,
  timeStartedAt,
  timeSpentSeconds,
  completedAt,
  planContent,
  planContentDraft,
  planEditing,
  planSaving,
  planHasChanges,
  employees,
  reviewerCandidates,
  coordinatorCandidates,
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
  onCoordinatorChange,
  onPlanEditStart,
  onPlanEditCancel,
  onPlanDraftChange,
  onPlanSave,
  onDeleteRequest,
}: TaskOverviewPanelProps) {
  const [timerNow, setTimerNow] = useState(() => Date.now());
  const elapsedSeconds = getTaskElapsedSeconds({
    time_started_at: timeStartedAt,
    time_spent_seconds: timeSpentSeconds,
  }, timerNow);
  const timerStatus = timeStartedAt
    ? "计时中"
    : completedAt
      ? "已完成"
      : timeSpentSeconds > 0
        ? "待继续"
        : "未开始";

  useEffect(() => {
    if (!timeStartedAt) {
      return;
    }

    setTimerNow(Date.now());
    const intervalId = window.setInterval(() => {
      setTimerNow(Date.now());
    }, 1000);

    return () => window.clearInterval(intervalId);
  }, [timeStartedAt]);

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
            <SelectTrigger className="h-7 w-[240px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === UNASSIGNED_VALUE) {
                    return "未指派";
                  }

                  const emp = employees.find((e) => e.id === value);
                  return emp ? `${emp.name} · ${emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}` : "未指派";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
              {employees.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}
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
            <SelectTrigger className="h-7 w-[240px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === UNASSIGNED_VALUE) {
                    return "未指定";
                  }

                  const emp = employees.find((e) => e.id === value);
                  return emp ? `${emp.name} · ${emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}` : "未指定";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
              {reviewerCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">协调员</span>
          <Select
            value={coordinatorId || UNASSIGNED_VALUE}
            onValueChange={(value) => onCoordinatorChange(!value || value === UNASSIGNED_VALUE ? "" : value)}
          >
            <SelectTrigger className="h-7 w-[240px] shrink-0 rounded-md px-2 text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === UNASSIGNED_VALUE) {
                    return "未指定";
                  }

                  const emp = coordinatorCandidates.find((e) => e.id === value);
                  return emp ? `${emp.name} · ${emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}` : "未指定";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
              {coordinatorCandidates.map((emp) => (
                <SelectItem key={emp.id} value={emp.id}>
                  {emp.name} · {emp.ai_provider === "claude" ? "Claude" : emp.ai_provider === "opencode" ? "OpenCode" : "Codex"}
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

      <section className="rounded-md border border-border bg-muted/20 p-3">
        <div className="mb-2 flex items-center gap-2 text-xs font-medium text-muted-foreground">
          <Clock className="h-3.5 w-3.5" />
          耗时汇总
        </div>
        <div className="grid gap-2 text-xs text-muted-foreground sm:grid-cols-2 lg:grid-cols-4">
          <div className="rounded-md border border-border bg-background/70 px-3 py-2">
            <span className="font-medium text-foreground">累计耗时：</span>
            {formatDuration(elapsedSeconds)}
          </div>
          <div className="rounded-md border border-border bg-background/70 px-3 py-2">
            <span className="font-medium text-foreground">计时开始：</span>
            {timeStartedAt ? formatDate(timeStartedAt) : "未开始"}
          </div>
          <div className="rounded-md border border-border bg-background/70 px-3 py-2">
            <span className="font-medium text-foreground">完成时间：</span>
            {completedAt ? formatDate(completedAt) : "未完成"}
          </div>
          <div className="rounded-md border border-border bg-background/70 px-3 py-2">
            <span className="font-medium text-foreground">计时状态：</span>
            {timerStatus}
          </div>
        </div>
        <div className="mt-2 text-[11px] text-muted-foreground">
          创建时间：{formatDate(createdAt)}
        </div>
      </section>

      <div>
        <label className="text-xs font-medium text-muted-foreground">
          描述
        </label>
        <Suspense fallback={<MonacoEditorFallback className="mt-1 h-[220px]" />}>
          <MonacoMarkdownEditor
            value={description}
            onChange={onDescriptionChange}
            onBlur={onDescriptionBlur}
            className="mt-1 h-[220px]"
            placeholder="添加任务描述..."
          />
        </Suspense>
      </div>

      <div>
        <div className="flex items-center justify-between gap-2">
          <label className="text-xs font-medium text-muted-foreground">
            计划内容
          </label>
          <div className="flex items-center gap-1">
            {planEditing && planHasChanges && (
              <button
                type="button"
                onClick={onPlanSave}
                disabled={planSaving}
                className="inline-flex h-7 items-center gap-1 rounded-md border border-input px-2 text-xs hover:bg-accent disabled:opacity-50"
                title="保存计划"
              >
                <Save className="h-3.5 w-3.5" />
                保存
              </button>
            )}
            {planEditing ? (
              <button
                type="button"
                onClick={onPlanEditCancel}
                disabled={planSaving}
                className="inline-flex h-7 items-center gap-1 rounded-md border border-input px-2 text-xs hover:bg-accent disabled:opacity-50"
                title="取消编辑"
              >
                <X className="h-3.5 w-3.5" />
                取消
              </button>
            ) : (
              <button
                type="button"
                onClick={onPlanEditStart}
                className="inline-flex h-7 items-center gap-1 rounded-md border border-input px-2 text-xs hover:bg-accent"
                title="编辑计划"
              >
                <Pencil className="h-3.5 w-3.5" />
                编辑
              </button>
            )}
          </div>
        </div>
        {planEditing ? (
          <Suspense fallback={<MonacoEditorFallback className="mt-1 h-72" />}>
            <MonacoMarkdownEditor
              value={planContentDraft}
              onChange={onPlanDraftChange}
              readOnly={planSaving}
              className="mt-1 h-72"
              placeholder="输入任务计划内容..."
            />
          </Suspense>
        ) : (
          <Suspense fallback={<MonacoEditorFallback className="mt-1 h-72" />}>
            <MonacoMarkdownEditor
              value={planContent}
              readOnly
              className="mt-1 h-72 bg-muted/30"
              placeholder="暂无计划内容"
            />
          </Suspense>
        )}
      </div>
    </div>
  );
}
