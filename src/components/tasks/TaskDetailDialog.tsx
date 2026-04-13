import { useState, useEffect, useRef } from "react";
import type { Task, TaskStatus } from "@/lib/types";
import { TASK_STATUSES, PRIORITIES } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { buildTaskExecutionPrompt } from "@/lib/taskPrompt";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Trash2, Sparkles, Loader2, Play, Square, Eraser } from "lucide-react";
import { aiSuggestAssignee, aiAnalyzeComplexity, aiGenerateComment, aiSplitSubtasks, startCodex, stopCodex } from "@/lib/codex";
import { SubtaskList } from "./SubtaskList";
import { CommentList } from "./CommentList";
import { DeleteTaskDialog } from "./DeleteTaskDialog";

const UNASSIGNED_VALUE = "__unassigned__";

interface TaskDetailDialogProps {
  task: Task;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function TaskDetailDialog({
  task,
  open,
  onOpenChange,
}: TaskDetailDialogProps) {
  const { updateTask, deleteTask, addComment, updateTaskStatus, fetchSubtasks, addSubtasks } = useTaskStore();
  const { employees, fetchEmployees, codexProcesses, updateEmployeeStatus, setCodexRunning, clearCodexOutput } = useEmployeeStore();
  const projects = useProjectStore((s) => s.projects);
  const projectRepoPath = projects.find((p) => p.id === task.project_id)?.repo_path;
  const [title, setTitle] = useState(task.title);
  const [description, setDescription] = useState(task.description ?? "");
  const [priority, setPriority] = useState(task.priority);
  const [status, setStatus] = useState(task.status);
  const [assigneeId, setAssigneeId] = useState(task.assignee_id ?? "");
  const [aiLoading, setAiLoading] = useState<string | null>(null);
  const [aiResult, setAiResult] = useState<string | null>(null);
  const [codexLoading, setCodexLoading] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingTask, setDeletingTask] = useState(false);
  const terminalRef = useRef<HTMLDivElement>(null);
  const assignee = assigneeId ? employees.find((employee) => employee.id === assigneeId) : undefined;

  const codexProcess = assigneeId ? codexProcesses[assigneeId] : undefined;
  const isRunning = (codexProcess?.running ?? false) && codexProcess?.activeTaskId === task.id;
  const output = codexProcess?.output ?? [];

  useEffect(() => {
    if (open) {
      fetchEmployees();
      setTitle(task.title);
      setDescription(task.description ?? "");
      setPriority(task.priority);
      setStatus(task.status);
      setAssigneeId(task.assignee_id ?? "");
      setAiResult(null);
    }
  }, [open, task]);

  useEffect(() => {
    if (!open) {
      setDeleteDialogOpen(false);
    }
  }, [open]);

  useEffect(() => {
    terminalRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [output.length]);

  const handleSave = async (field: string, value: string) => {
    if (field === "title" && value.trim()) {
      await updateTask(task.id, { title: value.trim() });
    } else if (field === "description") {
      await updateTask(task.id, { description: value || null });
    } else if (field === "priority") {
      await updateTask(task.id, { priority: value });
    } else if (field === "status") {
      await useTaskStore.getState().updateTaskStatus(task.id, value as TaskStatus);
    } else if (field === "assignee_id") {
      await updateTask(task.id, { assignee_id: value || null });
    }
  };

  const handleDelete = async () => {
    setDeletingTask(true);
    try {
      await deleteTask(task.id);
      setDeleteDialogOpen(false);
      onOpenChange(false);
    } catch (error) {
      console.error("Failed to delete task:", error);
    } finally {
      setDeletingTask(false);
    }
  };

  const handleRunCodex = async () => {
    if (!assigneeId) return;
    setCodexLoading(true);
    try {
      await updateEmployeeStatus(assigneeId, "busy");
      await updateTaskStatus(task.id, "in_progress");
      setStatus("in_progress");
      setCodexRunning(assigneeId, true, task.id);
      await fetchSubtasks(task.id);
      const desc = buildTaskExecutionPrompt({
        title,
        description,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
      });
      await startCodex(assigneeId, desc, {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir: projectRepoPath ?? undefined,
        taskId: task.id,
      });
    } catch (err) {
      console.error("Failed to start codex:", err);
      setCodexRunning(assigneeId, false, null);
      await updateEmployeeStatus(assigneeId, "error");
    } finally {
      setCodexLoading(false);
    }
  };

  const handleStopCodex = async () => {
    if (!assigneeId) return;
    setCodexLoading(true);
    try {
      await stopCodex(assigneeId);
      setCodexRunning(assigneeId, false, null);
      await updateEmployeeStatus(assigneeId, "offline");
    } catch (err) {
      console.error("Failed to stop codex:", err);
    } finally {
      setCodexLoading(false);
    }
  };

  const handleAiSuggest = async () => {
    setAiLoading("assignee");
    setAiResult(null);
    try {
      const employeeList = employees
        .map((e) => `${e.id}: ${e.name} (${e.role}, ${e.specialization ?? "general"})`)
        .join("; ");
      const desc = task.description ?? task.title;
      const result = await aiSuggestAssignee(desc, employeeList);
      setAiResult(result);
      await updateTask(task.id, { ai_suggestion: result });
    } catch (e) {
      setAiResult(`AI建议失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiComplexity = async () => {
    setAiLoading("complexity");
    setAiResult(null);
    try {
      const desc = task.description ?? task.title;
      const result = await aiAnalyzeComplexity(desc);
      setAiResult(result);
      const match = result.match(/(\d+)/);
      if (match) {
        await updateTask(task.id, { complexity: parseInt(match[1], 10) });
      }
    } catch (e) {
      setAiResult(`复杂度分析失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiComment = async () => {
    setAiLoading("comment");
    try {
      const result = await aiGenerateComment(
        task.title,
        task.description ?? "",
        `Status: ${task.status}, Priority: ${task.priority}`
      );
      await addComment(task.id, result, undefined, true);
    } catch (e) {
      console.error("AI comment failed:", e);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiSplitSubtasks = async () => {
    const taskTitle = title.trim();
    const taskDescription = description.trim();

    if (!taskTitle && !taskDescription) {
      setAiResult("请先填写任务标题或描述，再执行 AI 拆分。");
      return;
    }

    setAiLoading("subtasks");
    setAiResult(null);
    try {
      const generatedSubtasks = await aiSplitSubtasks(taskTitle, taskDescription);
      const { inserted, skipped } = await addSubtasks(task.id, generatedSubtasks);

      if (inserted === 0) {
        setAiResult(skipped > 0 ? "AI 已完成拆分，但结果与现有子任务重复，未新增内容。" : "AI 未生成可写入的子任务。");
        return;
      }

      setAiResult(`AI 已写入 ${inserted} 个子任务${skipped > 0 ? `，跳过 ${skipped} 个重复项` : ""}。`);
    } catch (e) {
      setAiResult(`AI拆分子任务失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  function getLineColor(line: string): string {
    if (line.startsWith("[ERROR]")) return "text-red-400";
    if (line.startsWith("[EXIT]")) return "text-yellow-400";
    return "text-green-400";
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="w-[min(96vw,80rem)] max-w-[min(96vw,80rem)] sm:max-w-[min(96vw,80rem)] max-h-[85vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="sr-only">任务详情</DialogTitle>
            <DialogDescription className="sr-only">
              查看和编辑任务详情
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            {/* Title */}
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onBlur={() => handleSave("title", title)}
              className="text-lg font-semibold border-none px-0 focus-visible:ring-0"
              placeholder="任务标题"
            />

            {/* Meta row */}
            <div className="flex flex-wrap items-center gap-3">
              {/* Status */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">状态</span>
                <Select
                  value={status}
                  onValueChange={(value) => {
                    if (!value) return;
                    setStatus(value);
                    handleSave("status", value);
                  }}
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
                    {TASK_STATUSES.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Priority */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">优先级</span>
                <Select
                  value={priority}
                  onValueChange={(value) => {
                    if (!value) return;
                    setPriority(value);
                    handleSave("priority", value);
                  }}
                >
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

              {/* Assignee */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">指派</span>
                <Select
                  value={assigneeId || UNASSIGNED_VALUE}
                  onValueChange={(value) => {
                    const nextValue =
                      !value || value === UNASSIGNED_VALUE ? "" : value;
                    setAssigneeId(nextValue);
                    handleSave("assignee_id", nextValue);
                  }}
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

              {/* Delete */}
              <button
                onClick={() => setDeleteDialogOpen(true)}
                disabled={isRunning || deletingTask}
                className="ml-auto p-1 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
                title={isRunning ? "运行中的任务不能删除，请先停止" : "删除任务"}
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>

          {/* Codex Run Controls */}
          <div className="flex items-center gap-2">
            {assigneeId ? (
              isRunning ? (
                <button
                  onClick={handleStopCodex}
                  disabled={codexLoading}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
                  停止运行
                </button>
              ) : (
                <button
                  onClick={handleRunCodex}
                  disabled={codexLoading}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50"
                >
                  {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
                  运行 Codex
                </button>
              )
            ) : (
              <span className="text-xs text-muted-foreground">请先指派员工以运行 Codex</span>
            )}
            {isRunning && (
              <span className="flex items-center gap-1 text-xs text-green-500">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                运行中
              </span>
            )}
          </div>

          {/* Codex Terminal Output */}
          {(isRunning || output.length > 0) && assigneeId && (
            <div>
              <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
                <span className="text-xs text-zinc-500 font-mono">Codex 终端</span>
                <button
                  onClick={() => clearCodexOutput(assigneeId)}
                  className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
                  title="清空日志"
                >
                  <Eraser className="h-3 w-3" />
                </button>
              </div>
              <ScrollArea className="h-40 bg-black rounded-b">
                <div className="p-2 font-mono text-xs space-y-0.5">
                  {output.length === 0 ? (
                    <div className="text-zinc-600">等待输出...</div>
                  ) : (
                    output.map((line, i) => (
                      <div key={i} className={`whitespace-pre-wrap ${getLineColor(line)}`}>
                        {line}
                      </div>
                    ))
                  )}
                  <div ref={terminalRef} />
                </div>
              </ScrollArea>
            </div>
          )}

          {/* AI Actions */}
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={handleAiSuggest}
              disabled={aiLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "assignee" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI建议指派
            </button>
            <button
              onClick={handleAiComplexity}
              disabled={aiLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "complexity" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              复杂度分析
            </button>
            <button
              onClick={handleAiSplitSubtasks}
              disabled={aiLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "subtasks" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI拆分子任务
            </button>
            <button
              onClick={handleAiComment}
              disabled={aiLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "comment" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI生成评论
            </button>
          </div>

          {/* AI Result */}
          {aiResult && (
            <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
              <span className="font-medium text-primary">AI 结果: </span>
              {aiResult}
            </div>
          )}

          {/* AI Suggestion (persisted) */}
          {task.ai_suggestion && !aiResult && (
            <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
              <span className="font-medium text-primary">AI 建议: </span>
              {task.ai_suggestion}
            </div>
          )}

          <Separator />

          {/* Description */}
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              描述
            </label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onBlur={() => handleSave("description", description)}
              className="mt-1 min-h-[80px] resize-y"
              placeholder="添加任务描述..."
            />
          </div>

          <Separator />

          {/* Subtasks */}
          <SubtaskList taskId={task.id} />

          <Separator />

          {/* Comments */}
          <CommentList taskId={task.id} />
          </div>
        </DialogContent>
      </Dialog>
      <DeleteTaskDialog
        open={deleteDialogOpen}
        task={task}
        deleting={deletingTask}
        onOpenChange={setDeleteDialogOpen}
        onConfirm={handleDelete}
      />
    </>
  );
}
