import { useEffect, useState } from "react";
import { AlertTriangle, Link2, Plus, Tag as TagIcon, X } from "lucide-react";

import type { Milestone, Tag, Task, TaskDependency } from "@/lib/types";
import {
  addTaskDependency,
  createTag,
  listMilestones,
  listTags,
  listTaskDependencies,
  listTaskTags,
  removeTaskDependency,
  setTaskTags,
} from "@/lib/backend";
import { formatDate, getDateOnly, isTaskOverdue } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const NONE_VALUE = "__none__";

interface TaskDeliverySectionProps {
  task: Task;
  projectTasks: Task[];
  dueDate: string;
  milestoneId: string;
  onDueDateChange: (value: string) => void;
  onDueDateBlur: () => void;
  onMilestoneChange: (value: string) => void;
  onError?: (message: string) => void;
}

export function TaskDeliverySection({
  task,
  projectTasks,
  dueDate,
  milestoneId,
  onDueDateChange,
  onDueDateBlur,
  onMilestoneChange,
  onError,
}: TaskDeliverySectionProps) {
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [projectTags, setProjectTags] = useState<Tag[]>([]);
  const [taskTags, setTaskTagsState] = useState<Tag[]>([]);
  const [dependencies, setDependencies] = useState<TaskDependency[]>([]);
  const [newTagName, setNewTagName] = useState("");
  const [tagBusy, setTagBusy] = useState(false);
  const [depBusy, setDepBusy] = useState(false);
  const overdue = isTaskOverdue({ due_date: dueDate || null, status: task.status });

  const reloadDeliveryMeta = async () => {
    try {
      const [nextMilestones, nextTags, nextTaskTags, nextDeps] = await Promise.all([
        listMilestones(task.project_id),
        listTags(task.project_id),
        listTaskTags(task.id),
        listTaskDependencies(task.id),
      ]);
      setMilestones(nextMilestones);
      setProjectTags(nextTags);
      setTaskTagsState(nextTaskTags);
      setDependencies(nextDeps);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    }
  };

  useEffect(() => {
    void reloadDeliveryMeta();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [task.id, task.project_id]);

  const availableTags = projectTags.filter(
    (tag) => !taskTags.some((current) => current.id === tag.id),
  );
  const availableDeps = projectTasks.filter(
    (item) =>
      item.id !== task.id && !dependencies.some((dep) => dep.depends_on_task_id === item.id),
  );

  const handleAddExistingTag = async (tagId: string) => {
    if (!tagId || tagId === NONE_VALUE) {
      return;
    }
    setTagBusy(true);
    try {
      const next = await setTaskTags({
        task_id: task.id,
        tag_ids: [...taskTags.map((tag) => tag.id), tagId],
      });
      setTaskTagsState(next);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    } finally {
      setTagBusy(false);
    }
  };

  const handleCreateTag = async () => {
    const name = newTagName.trim();
    if (!name) {
      return;
    }
    setTagBusy(true);
    try {
      const created = await createTag({ project_id: task.project_id, name });
      const next = await setTaskTags({
        task_id: task.id,
        tag_ids: [...taskTags.map((tag) => tag.id), created.id],
      });
      setProjectTags((current) => [...current, created]);
      setTaskTagsState(next);
      setNewTagName("");
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    } finally {
      setTagBusy(false);
    }
  };

  const handleRemoveTag = async (tagId: string) => {
    setTagBusy(true);
    try {
      const next = await setTaskTags({
        task_id: task.id,
        tag_ids: taskTags.filter((tag) => tag.id !== tagId).map((tag) => tag.id),
      });
      setTaskTagsState(next);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    } finally {
      setTagBusy(false);
    }
  };

  const handleAddDependency = async (dependsOnTaskId: string) => {
    if (!dependsOnTaskId || dependsOnTaskId === NONE_VALUE) {
      return;
    }
    setDepBusy(true);
    try {
      const created = await addTaskDependency({
        task_id: task.id,
        depends_on_task_id: dependsOnTaskId,
      });
      setDependencies((current) => [...current, created]);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    } finally {
      setDepBusy(false);
    }
  };

  const handleRemoveDependency = async (dependencyId: string) => {
    setDepBusy(true);
    try {
      await removeTaskDependency(dependencyId);
      setDependencies((current) => current.filter((dep) => dep.id !== dependencyId));
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
    } finally {
      setDepBusy(false);
    }
  };

  return (
    <section className="space-y-3 rounded-md border border-border bg-muted/20 p-3">
      <h4 className="text-xs font-medium text-muted-foreground">交付信息</h4>

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <label className="text-xs font-medium text-muted-foreground">截止日期</label>
          <Input
            type="date"
            value={getDateOnly(dueDate) ?? ""}
            onChange={(e) => onDueDateChange(e.target.value)}
            onBlur={onDueDateBlur}
            className={`mt-1 h-8 text-xs ${overdue ? "border-destructive text-destructive" : ""}`}
          />
          {dueDate && (
            <p
              className={`mt-1 text-[11px] ${overdue ? "text-destructive" : "text-muted-foreground"}`}
            >
              {overdue ? "已逾期 · " : ""}
              {formatDate(dueDate)}
            </p>
          )}
        </div>

        <div>
          <label className="text-xs font-medium text-muted-foreground">里程碑</label>
          <Select
            value={milestoneId || NONE_VALUE}
            onValueChange={(value) =>
              onMilestoneChange(!value || value === NONE_VALUE ? "" : value)
            }
          >
            <SelectTrigger className="mt-1 h-8 bg-background text-xs">
              <SelectValue>
                {(value) => {
                  if (!value || value === NONE_VALUE) {
                    return "未关联";
                  }
                  return milestones.find((item) => item.id === value)?.name ?? "未关联";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE_VALUE}>未关联</SelectItem>
              {milestones.map((milestone) => (
                <SelectItem key={milestone.id} value={milestone.id}>
                  {milestone.name}
                  {milestone.due_date ? `（${formatDate(milestone.due_date)}）` : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          <TagIcon className="h-3.5 w-3.5" />
          标签
        </div>
        <div className="flex flex-wrap gap-1.5">
          {taskTags.length === 0 && (
            <span className="text-[11px] text-muted-foreground">暂无标签</span>
          )}
          {taskTags.map((tag) => (
            <Badge key={tag.id} variant="secondary" className="gap-1 pr-1">
              <span
                className="h-2 w-2 rounded-full"
                style={{ backgroundColor: tag.color || "var(--primary)" }}
              />
              {tag.name}
              <button
                type="button"
                disabled={tagBusy}
                onClick={() => void handleRemoveTag(tag.id)}
                className="rounded-full p-0.5 hover:bg-muted"
                title="移除标签"
              >
                <X className="h-3 w-3" />
              </button>
            </Badge>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select
            disabled={tagBusy || availableTags.length === 0}
            value={NONE_VALUE}
            onValueChange={(value) => value && void handleAddExistingTag(value)}
          >
            <SelectTrigger className="h-7 w-[160px] bg-background text-xs">
              <SelectValue placeholder="添加已有标签">添加已有标签</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {availableTags.map((tag) => (
                <SelectItem key={tag.id} value={tag.id}>
                  {tag.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Input
            value={newTagName}
            onChange={(e) => setNewTagName(e.target.value)}
            placeholder="新建标签"
            className="h-7 w-[140px] text-xs"
            disabled={tagBusy}
          />
          <button
            type="button"
            disabled={tagBusy || !newTagName.trim()}
            onClick={() => void handleCreateTag()}
            className="inline-flex h-7 items-center gap-1 rounded-md border border-input px-2 text-xs hover:bg-accent disabled:opacity-50"
          >
            <Plus className="h-3 w-3" />
            创建并添加
          </button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          <Link2 className="h-3.5 w-3.5" />
          依赖任务
        </div>
        <div className="space-y-1.5">
          {dependencies.length === 0 && (
            <p className="text-[11px] text-muted-foreground">暂无依赖</p>
          )}
          {dependencies.map((dep) => {
            const dependsOn = projectTasks.find((item) => item.id === dep.depends_on_task_id);
            const incomplete = dependsOn && dependsOn.status !== "completed";
            return (
              <div
                key={dep.id}
                className="flex items-start justify-between gap-2 rounded-md border border-border bg-background/70 px-2.5 py-1.5 text-xs"
              >
                <div className="min-w-0">
                  <p className="truncate font-medium">
                    {dependsOn?.title ?? dep.depends_on_task_id}
                  </p>
                  {incomplete && (
                    <p className="mt-0.5 flex items-center gap-1 text-[11px] text-amber-700 dark:text-amber-200">
                      <AlertTriangle className="h-3 w-3 shrink-0" />
                      依赖任务尚未完成
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  disabled={depBusy}
                  onClick={() => void handleRemoveDependency(dep.id)}
                  className="shrink-0 rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-destructive disabled:opacity-50"
                  title="移除依赖"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
            );
          })}
        </div>
        <Select
          disabled={depBusy || availableDeps.length === 0}
          value={NONE_VALUE}
          onValueChange={(value) => value && void handleAddDependency(value)}
        >
          <SelectTrigger className="h-7 bg-background text-xs">
            <SelectValue placeholder="添加依赖任务">添加依赖任务</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {availableDeps.map((item) => (
              <SelectItem key={item.id} value={item.id}>
                {item.title}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </section>
  );
}
