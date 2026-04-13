import { useEffect, useState } from "react";
import { startCodex, stopCodex, restartCodex } from "@/lib/codex";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { buildTaskExecutionPrompt } from "@/lib/taskPrompt";
import { cn, getPriorityLabel, getStatusLabel } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import type { Task } from "@/lib/types";
import { Play, Square, RotateCw, Loader2, Search } from "lucide-react";

const STARTABLE_TASK_STATUSES = ["todo", "in_progress", "review"];

interface CodexControlsProps {
  employeeId: string;
  employeeStatus: string;
  model: string;
  reasoningEffort: string;
  systemPrompt?: string | null;
}

export function CodexControls({ employeeId, employeeStatus, model, reasoningEffort, systemPrompt }: CodexControlsProps) {
  const updateEmployeeStatus = useEmployeeStore((s) => s.updateEmployeeStatus);
  const setCodexRunning = useEmployeeStore((s) => s.setCodexRunning);
  const tasks = useTaskStore((s) => s.tasks);
  const fetchTasks = useTaskStore((s) => s.fetchTasks);
  const fetchSubtasks = useTaskStore((s) => s.fetchSubtasks);
  const updateTask = useTaskStore((s) => s.updateTask);
  const updateTaskStatus = useTaskStore((s) => s.updateTaskStatus);
  const projects = useProjectStore((s) => s.projects);
  const fetchProjects = useProjectStore((s) => s.fetchProjects);
  const [taskDescription, setTaskDescription] = useState("");
  const [actionLoading, setActionLoading] = useState<"start" | "stop" | "restart" | null>(null);
  const [showTaskDialog, setShowTaskDialog] = useState(false);
  const [taskDialogLoading, setTaskDialogLoading] = useState(false);
  const [taskKeyword, setTaskKeyword] = useState("");
  const [selectedTaskId, setSelectedTaskId] = useState("");

  const isRunning = employeeStatus === "online" || employeeStatus === "busy";
  const normalizedKeyword = taskKeyword.trim().toLowerCase();
  const eligibleTasks = tasks.filter((task) => {
    if (!STARTABLE_TASK_STATUSES.includes(task.status)) {
      return false;
    }

    if (task.assignee_id && task.assignee_id !== employeeId) {
      return false;
    }

    if (!normalizedKeyword) {
      return true;
    }

    const projectName = projects.find((project) => project.id === task.project_id)?.name ?? "";
    const searchableText = [task.title, task.description ?? "", projectName].join(" ").toLowerCase();
    return searchableText.includes(normalizedKeyword);
  });
  const selectedTask = eligibleTasks.find((task) => task.id === selectedTaskId) ?? null;

  useEffect(() => {
    if (!showTaskDialog) {
      setTaskKeyword("");
      setSelectedTaskId("");
      return;
    }

    let cancelled = false;

    setTaskDialogLoading(true);
    void Promise.all([fetchTasks(), fetchProjects()])
      .catch((error) => {
        console.error("Failed to load tasks for employee start:", error);
      })
      .finally(() => {
        if (!cancelled) {
          setTaskDialogLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [showTaskDialog, fetchProjects, fetchTasks]);

  useEffect(() => {
    if (!showTaskDialog) {
      return;
    }

    if (!selectedTaskId || !eligibleTasks.some((task) => task.id === selectedTaskId)) {
      setSelectedTaskId(eligibleTasks[0]?.id ?? "");
    }
  }, [eligibleTasks, selectedTaskId, showTaskDialog]);

  const getProjectName = (task: Task) => (
    projects.find((project) => project.id === task.project_id)?.name ?? "未命名项目"
  );

  const getProjectRepoPath = (task: Task) => (
    projects.find((project) => project.id === task.project_id)?.repo_path ?? undefined
  );

  const handleStart = async () => {
    if (!selectedTask) {
      return;
    }

    setActionLoading("start");
    try {
      if (!selectedTask.assignee_id) {
        await updateTask(selectedTask.id, { assignee_id: employeeId });
      }

      await updateEmployeeStatus(employeeId, "busy");
      await updateTaskStatus(selectedTask.id, "in_progress");
      setCodexRunning(employeeId, true, selectedTask.id);
      await fetchSubtasks(selectedTask.id);

      const prompt = buildTaskExecutionPrompt({
        title: selectedTask.title,
        description: selectedTask.description,
        subtasks: useTaskStore.getState().subtasks[selectedTask.id] ?? [],
      });

      await startCodex(employeeId, prompt, {
        model,
        reasoningEffort,
        systemPrompt,
        workingDir: getProjectRepoPath(selectedTask),
        taskId: selectedTask.id,
      });
      setShowTaskDialog(false);
    } catch (e) {
      console.error("Failed to start codex:", e);
      setCodexRunning(employeeId, false, null);
      await updateEmployeeStatus(employeeId, "error");
    } finally {
      setActionLoading(null);
    }
  };

  const handleStop = async () => {
    setActionLoading("stop");
    try {
      await stopCodex(employeeId);
      setCodexRunning(employeeId, false, null);
    } catch (e) {
      console.error("Failed to stop codex:", e);
    }
    await updateEmployeeStatus(employeeId, "offline");
    setActionLoading(null);
  };

  const handleRestart = async () => {
    if (!taskDescription.trim()) {
      return;
    }
    setActionLoading("restart");
    try {
      setCodexRunning(employeeId, true, null);
      await restartCodex(employeeId, taskDescription.trim(), { model, reasoningEffort, systemPrompt });
      setTaskDescription("");
    } catch (e) {
      console.error("Failed to restart codex:", e);
      setCodexRunning(employeeId, false, null);
    } finally {
      setActionLoading(null);
    }
  };

  return (
    <>
      <div className="flex items-center gap-1.5">
        {!isRunning ? (
          <button
            onClick={() => setShowTaskDialog(true)}
            disabled={actionLoading !== null}
            className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 transition-colors"
          >
            {actionLoading === "start" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
            启动
          </button>
        ) : (
          <>
            <button
              onClick={handleStop}
              disabled={actionLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50 transition-colors"
            >
              {actionLoading === "stop" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
              停止
            </button>
            <button
              onClick={handleRestart}
              disabled={actionLoading !== null}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-yellow-600 text-white rounded hover:bg-yellow-700 disabled:opacity-50 transition-colors"
            >
              {actionLoading === "restart" ? <Loader2 className="h-3 w-3 animate-spin" /> : <RotateCw className="h-3 w-3" />}
              重启
            </button>
          </>
        )}

        {isRunning && (
          <input
            type="text"
            value={taskDescription}
            onChange={(e) => setTaskDescription(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleRestart()}
            placeholder="新任务描述（重启用）"
            className="flex-1 px-2 py-1 text-xs border border-input rounded bg-background ml-1"
          />
        )}
      </div>
      <Dialog
        open={showTaskDialog}
        onOpenChange={(open) => {
          if (actionLoading === "start") {
            return;
          }
          setShowTaskDialog(open);
        }}
      >
        <DialogContent className="w-[min(96vw,40rem)] max-w-[min(96vw,40rem)] sm:max-w-[min(96vw,40rem)]">
          <DialogHeader>
            <DialogTitle>选择启动任务</DialogTitle>
            <DialogDescription>
              只显示未指派任务，或已指派给当前员工且状态为待办、进行中、审核中的任务。
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={taskKeyword}
                onChange={(e) => setTaskKeyword(e.target.value)}
                placeholder="搜索任务标题、描述或项目"
                className="pl-8"
                autoFocus
              />
            </div>

            <ScrollArea className="h-72 rounded-lg border border-border">
              <div className="space-y-2 p-2">
                {taskDialogLoading ? (
                  <div className="flex h-60 items-center justify-center text-sm text-muted-foreground">
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    正在加载任务...
                  </div>
                ) : eligibleTasks.length === 0 ? (
                  <div className="flex h-60 items-center justify-center text-sm text-muted-foreground">
                    没有符合条件的任务
                  </div>
                ) : (
                  eligibleTasks.map((task) => (
                    <button
                      key={task.id}
                      type="button"
                      onClick={() => setSelectedTaskId(task.id)}
                      className={cn(
                        "w-full rounded-lg border p-3 text-left transition-colors",
                        selectedTaskId === task.id
                          ? "border-primary bg-primary/5"
                          : "border-border hover:bg-accent/50"
                      )}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="truncate text-sm font-medium">{task.title}</div>
                          {task.description && (
                            <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                              {task.description}
                            </p>
                          )}
                        </div>
                        <div className="shrink-0 rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground">
                          {task.assignee_id ? "已指派" : "未指派"}
                        </div>
                      </div>
                      <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                        <span>{getStatusLabel(task.status)}</span>
                        <span>{getPriorityLabel(task.priority)}</span>
                        <span>{getProjectName(task)}</span>
                      </div>
                    </button>
                  ))
                )}
              </div>
            </ScrollArea>

            {selectedTask && (
              <div className="rounded-lg border border-border bg-muted/40 p-3 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">{selectedTask.title}</div>
                <div className="mt-1">
                  {selectedTask.assignee_id ? "将继续使用当前员工负责该任务。" : "启动时会自动把该任务指派给当前员工。"}
                </div>
              </div>
            )}
          </div>

          <DialogFooter className="mt-1">
            <button
              type="button"
              onClick={() => setShowTaskDialog(false)}
              disabled={actionLoading === "start"}
              className="h-8 rounded-lg border border-border bg-background px-3 text-sm hover:bg-muted disabled:opacity-50"
            >
              取消
            </button>
            <button
              type="button"
              onClick={handleStart}
              disabled={!selectedTask || taskDialogLoading || actionLoading === "start"}
              className="flex h-8 items-center justify-center gap-1 rounded-lg bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {actionLoading === "start" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
              启动任务
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
