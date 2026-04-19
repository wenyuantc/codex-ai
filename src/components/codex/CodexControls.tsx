import { useEffect, useMemo, useState } from "react";
import { Loader2, Play, Search, Square } from "lucide-react";

import { prepareTaskGitExecution, startTaskCodeReview } from "@/lib/backend";
import { startCodex, stopCodex } from "@/lib/codex";
import { getProjectWorkingDir } from "@/lib/projects";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import type { Task } from "@/lib/types";
import { cn, getPriorityLabel, getStatusLabel } from "@/lib/utils";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";

const STARTABLE_TASK_STATUSES = ["todo", "in_progress", "review"];

interface CodexControlsProps {
  employeeId: string;
  employeeRole: string;
  employeeStatus: string;
  model: string;
  reasoningEffort: string;
  systemPrompt?: string | null;
}

export function CodexControls({
  employeeId,
  employeeRole,
  employeeStatus: _employeeStatus,
  model,
  reasoningEffort,
  systemPrompt,
}: CodexControlsProps) {
  const employeeRuntime = useEmployeeStore((state) => state.employeeRuntime[employeeId]);
  const allEmployeeRuntime = useEmployeeStore((state) => state.employeeRuntime);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const refreshEmployeeRuntimeStatus = useEmployeeStore((state) => state.refreshEmployeeRuntimeStatus);
  const tasks = useTaskStore((state) => state.tasks);
  const fetchTasks = useTaskStore((state) => state.fetchTasks);
  const fetchAttachments = useTaskStore((state) => state.fetchAttachments);
  const fetchSubtasks = useTaskStore((state) => state.fetchSubtasks);
  const updateTask = useTaskStore((state) => state.updateTask);
  const updateTaskStatus = useTaskStore((state) => state.updateTaskStatus);
  const projects = useProjectStore((state) => state.projects);
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const fetchProjects = useProjectStore((state) => state.fetchProjects);
  const [actionLoading, setActionLoading] = useState<"start" | "stop" | null>(null);
  const [showTaskDialog, setShowTaskDialog] = useState(false);
  const [taskDialogLoading, setTaskDialogLoading] = useState(false);
  const [taskKeyword, setTaskKeyword] = useState("");
  const [selectedTaskId, setSelectedTaskId] = useState("");

  const isReviewer = employeeRole === "reviewer";
  const runningSessions = employeeRuntime?.sessions ?? [];
  const hasRunningSessions = runningSessions.length > 0;
  const allRunningSessions = useMemo(
    () => Object.values(allEmployeeRuntime).flatMap((runtime) => runtime.sessions),
    [allEmployeeRuntime],
  );
  const normalizedKeyword = taskKeyword.trim().toLowerCase();
  const eligibleTasks = tasks.filter((task) => {
    if (currentProjectId && task.project_id !== currentProjectId) {
      return false;
    }

    if (!STARTABLE_TASK_STATUSES.includes(task.status)) {
      return false;
    }

    const reviewerCanPickReviewTask = isReviewer && task.status === "review";
    if (!reviewerCanPickReviewTask && task.assignee_id && task.assignee_id !== employeeId) {
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
  const shouldStartReview = isReviewer && selectedTask?.status === "review";
  const selectedTaskRunningSession = useMemo(() => {
    if (!selectedTask) {
      return null;
    }

    const expectedSessionKind = shouldStartReview ? "review" : "execution";
    return allRunningSessions.find((session) => (
      session.task_id === selectedTask.id && session.session_kind === expectedSessionKind
    )) ?? null;
  }, [allRunningSessions, selectedTask, shouldStartReview]);

  useEffect(() => {
    if (!showTaskDialog) {
      setTaskKeyword("");
      setSelectedTaskId("");
      return;
    }

    let cancelled = false;
    setTaskDialogLoading(true);
    void Promise.all([fetchTasks(currentProjectId), fetchProjects()])
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
  }, [showTaskDialog, currentProjectId, fetchProjects, fetchTasks]);

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
    getProjectWorkingDir(projects.find((project) => project.id === task.project_id)) ?? undefined
  );

  const handleStart = async () => {
    if (!selectedTask) {
      return;
    }

    const startAsReview = isReviewer && selectedTask.status === "review";
    const sessionKind = startAsReview ? "review" : "execution";
    let reviewerReassigned = false;
    setActionLoading("start");

    try {
      if (startAsReview) {
        if (selectedTask.reviewer_id !== employeeId) {
          await updateTask(selectedTask.id, { reviewer_id: employeeId });
          reviewerReassigned = true;
        }

        await updateEmployeeStatus(employeeId, "busy");
        clearTaskCodexOutput(selectedTask.id, "review");
        await startTaskCodeReview(selectedTask.id);
        await refreshEmployeeRuntimeStatus(employeeId);
        setShowTaskDialog(false);
        return;
      }

      if (!selectedTask.assignee_id) {
        await updateTask(selectedTask.id, { assignee_id: employeeId });
      }

      await updateEmployeeStatus(employeeId, "busy");
      await updateTaskStatus(selectedTask.id, "in_progress");
      clearTaskCodexOutput(selectedTask.id);
      await Promise.all([fetchSubtasks(selectedTask.id), fetchAttachments(selectedTask.id)]);

      const executionInput = buildTaskExecutionInput({
        title: selectedTask.title,
        description: selectedTask.description,
        subtasks: useTaskStore.getState().subtasks[selectedTask.id] ?? [],
        attachments: useTaskStore.getState().attachments[selectedTask.id] ?? [],
      });
      let workingDir = getProjectRepoPath(selectedTask);
      let taskGitContextId: string | undefined;

      if (selectedTask.use_worktree) {
        const prepared = await prepareTaskGitExecution(selectedTask.id);
        workingDir = prepared.working_dir;
        taskGitContextId = prepared.task_git_context_id;
      }

      await startCodex(employeeId, executionInput.prompt, {
        model,
        reasoningEffort,
        systemPrompt,
        workingDir,
        taskId: selectedTask.id,
        taskGitContextId,
        imagePaths: executionInput.imagePaths,
      });
      await refreshEmployeeRuntimeStatus(employeeId);
      setShowTaskDialog(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("Failed to start task:", error);
      if (reviewerReassigned) {
        try {
          await updateTask(selectedTask.id, { reviewer_id: selectedTask.reviewer_id ?? null });
        } catch (rollbackError) {
          console.error("Failed to rollback reviewer assignment:", rollbackError);
        }
      }
      addCodexOutput(employeeId, `[ERROR] ${message}`, selectedTask.id, sessionKind);
      const runtime = await refreshEmployeeRuntimeStatus(employeeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(employeeId, "error");
      }
    } finally {
      setActionLoading(null);
    }
  };

  const handleStopAll = async () => {
    if (!hasRunningSessions) {
      return;
    }

    setActionLoading("stop");
    try {
      await stopCodex(employeeId);
      const runtime = await refreshEmployeeRuntimeStatus(employeeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(employeeId, "offline");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const fallbackSession = runningSessions[0] ?? null;
      addCodexOutput(
        employeeId,
        `[ERROR] ${message}`,
        fallbackSession?.task_id ?? null,
        fallbackSession?.session_kind ?? "execution",
        fallbackSession?.session_record_id ?? null,
      );
      const runtime = await refreshEmployeeRuntimeStatus(employeeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(employeeId, "error");
      }
    } finally {
      setActionLoading(null);
    }
  };

  return (
    <>
      <div className="flex items-center gap-1.5">
        <button
          onClick={() => setShowTaskDialog(true)}
          disabled={actionLoading !== null}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 transition-colors"
        >
          {actionLoading === "start" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
          启动
        </button>
        {hasRunningSessions && (
          <button
            onClick={() => void handleStopAll()}
            disabled={actionLoading !== null}
            title={`停止当前员工的 ${runningSessions.length} 个运行会话`}
            className="flex items-center gap-1 px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50 transition-colors"
          >
            {actionLoading === "stop" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
            停止全部
          </button>
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
              {isReviewer
                ? "显示未指派任务、已指派给当前员工的任务，以及当前项目下全部审核中的任务。"
                : "只显示未指派任务，或已指派给当前员工且状态为待办、进行中、审核中的任务。"}
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
                          : "border-border hover:bg-accent/50",
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
                  {selectedTaskRunningSession
                    ? "该任务当前已有同类型运行会话，不能重复启动。"
                    : shouldStartReview
                      ? selectedTask.reviewer_id === employeeId
                        ? "将以当前审查员身份发起该任务的代码审核。"
                        : "启动时会自动将该任务改派给当前审查员，并发起代码审核。"
                      : selectedTask.assignee_id
                        ? "将继续使用当前员工负责该任务。"
                        : "启动时会自动把该任务指派给当前员工。"}
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
              disabled={!selectedTask || Boolean(selectedTaskRunningSession) || taskDialogLoading || actionLoading === "start"}
              className="flex h-8 items-center justify-center gap-1 rounded-lg bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {actionLoading === "start" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
              {shouldStartReview ? "启动审核" : "启动任务"}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
