import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Task } from "@/lib/types";
import { getPriorityLabel, getPriorityColor, formatDate } from "@/lib/utils";
import { buildTaskExecutionPrompt } from "@/lib/taskPrompt";
import { Clock, FolderKanban, GripVertical, MessageSquarePlus, Play, ScrollText, Square, Trash2 } from "lucide-react";
import { ContinueConversationDialog } from "./ContinueConversationDialog";
import { TaskDetailDialog } from "./TaskDetailDialog";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";
import { startCodex, stopCodex } from "@/lib/codex";

interface TaskCardProps {
  task: Task;
  isOverlay?: boolean;
  onOpenLog?: (taskId: string) => void;
}

export function TaskCard({ task, isOverlay, onOpenLog }: TaskCardProps) {
  const [showDetail, setShowDetail] = useState(false);
  const [showContinueDialog, setShowContinueDialog] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [actionLoading, setActionLoading] = useState<"run" | "stop" | "continue" | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const projects = useProjectStore((s) => s.projects);
  const employees = useEmployeeStore((s) => s.employees);
  const projectName = projects.find((p) => p.id === task.project_id)?.name;
  const projectRepoPath = projects.find((p) => p.id === task.project_id)?.repo_path;
  const codexProcesses = useEmployeeStore((s) => s.codexProcesses);
  const updateEmployeeStatus = useEmployeeStore((s) => s.updateEmployeeStatus);
  const setCodexRunning = useEmployeeStore((s) => s.setCodexRunning);
  const addCodexOutput = useEmployeeStore((s) => s.addCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((s) => s.clearTaskCodexOutput);
  const refreshCodexRuntimeStatus = useEmployeeStore((s) => s.refreshCodexRuntimeStatus);
  const updateTaskStatus = useTaskStore((s) => s.updateTaskStatus);
  const fetchSubtasks = useTaskStore((s) => s.fetchSubtasks);
  const deleteTask = useTaskStore((s) => s.deleteTask);
  const assignee = task.assignee_id ? employees.find((employee) => employee.id === task.assignee_id) : undefined;

  const isRunning = task.assignee_id
    ? (codexProcesses[task.assignee_id]?.running ?? false)
      && codexProcesses[task.assignee_id]?.activeTaskId === task.id
    : false;

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: task.id,
    data: { type: "task", status: task.status },
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  useEffect(() => {
    if (!contextMenu) return;

    const handleClose = () => setContextMenu(null);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };

    window.addEventListener("resize", handleClose);
    document.addEventListener("scroll", handleClose, true);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("resize", handleClose);
      document.removeEventListener("scroll", handleClose, true);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [contextMenu]);

  const startTaskSession = async ({
    prompt,
    mode,
    resumeSessionId,
  }: {
    prompt: string;
    mode: "run" | "continue";
    resumeSessionId?: string;
  }) => {
    if (!task.assignee_id) return;

    setActionLoading(mode);
    try {
      await updateEmployeeStatus(task.assignee_id, "busy");
      await updateTaskStatus(task.id, "in_progress");
      setCodexRunning(task.assignee_id, true, task.id);
      await startCodex(task.assignee_id, prompt, {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir: projectRepoPath ?? undefined,
        taskId: task.id,
        resumeSessionId,
      });
      setShowContinueDialog(false);
    } catch (err) {
      console.error(`Failed to ${mode === "continue" ? "resume" : "start"} codex:`, err);
      addCodexOutput(task.assignee_id, `[ERROR] ${String(err)}`, task.id);
      setCodexRunning(task.assignee_id, false, null);
      await refreshCodexRuntimeStatus(task.assignee_id);
    } finally {
      setActionLoading(null);
      setContextMenu(null);
    }
  };

  const handleRun = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setContextMenu(null);
    onOpenLog?.(task.id);
    clearTaskCodexOutput(task.id);
    await fetchSubtasks(task.id);
    const desc = buildTaskExecutionPrompt({
      title: task.title,
      description: task.description,
      subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
    });
    await startTaskSession({
      prompt: desc,
      mode: "run",
    });
  };

  const handleStop = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    if (!task.assignee_id) return;
    setActionLoading("stop");
    try {
      await stopCodex(task.assignee_id);
      setCodexRunning(task.assignee_id, false, null);
      await updateEmployeeStatus(task.assignee_id, "offline");
    } catch (err) {
      console.error("Failed to stop codex:", err);
    } finally {
      setActionLoading(null);
      setContextMenu(null);
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deleteTask(task.id);
      setShowDeleteDialog(false);
      setShowDetail(false);
      setContextMenu(null);
    } catch (error) {
      console.error("Failed to delete task:", error);
    } finally {
      setDeleting(false);
    }
  };

  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    if (isOverlay) return;
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      x: Math.max(8, Math.min(e.clientX, window.innerWidth - 184)),
      y: Math.max(8, Math.min(e.clientY, window.innerHeight - 120)),
    });
  };

  const openDeleteDialog = () => {
    if (isRunning) return;
    setContextMenu(null);
    setShowDeleteDialog(true);
  };

  const openLogDialog = () => {
    setContextMenu(null);
    onOpenLog?.(task.id);
  };

  const openContinueDialog = () => {
    if (!task.last_codex_session_id || isRunning) return;
    setContextMenu(null);
    setShowContinueDialog(true);
  };

  const handleContinueConversation = async (prompt: string) => {
    if (!task.last_codex_session_id) return;
    await fetchSubtasks(task.id);
    await startTaskSession({
      prompt: buildTaskExecutionPrompt({
        title: task.title,
        description: task.description,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
        followUpPrompt: prompt,
      }),
      mode: "continue",
      resumeSessionId: task.last_codex_session_id,
    });
  };

  return (
    <>
      <div
        ref={setNodeRef}
        style={style}
        className={`bg-card rounded-md border border-border p-3 group ${
          isDragging
            ? "opacity-50 shadow-lg"
            : "hover:shadow-sm cursor-pointer"
        } transition-shadow`}
        onClick={() => !isDragging && setShowDetail(true)}
        onContextMenu={handleContextMenu}
        {...attributes}
      >
        <div className="flex items-start gap-2">
          <button
            className="mt-0.5 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing shrink-0"
            {...listeners}
            onClick={(e) => e.stopPropagation()}
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium truncate">{task.title}</p>
            {task.description && (
              <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                {task.description}
              </p>
            )}
            <div className="flex items-center gap-2 mt-2 flex-wrap">
              <span
                className={`text-xs font-medium ${getPriorityColor(
                  task.priority
                )}`}
              >
                {getPriorityLabel(task.priority)}
              </span>
              {task.complexity && (
                <span className="text-xs text-muted-foreground">
                  复杂度: {task.complexity}/10
                </span>
              )}
            </div>
            <div className="flex items-center justify-between mt-1.5 text-xs text-muted-foreground">
              <div className="flex flex-col gap-0.5">
                {projectName && (
                  <span className="flex items-center gap-0.5">
                    <FolderKanban className="h-3 w-3" />
                    {projectName}
                  </span>
                )}
                <span className="flex items-center gap-0.5">
                  <Clock className="h-3 w-3" />
                  {formatDate(task.created_at)}
                </span>
              </div>
              {task.assignee_id && (
                <span className="inline-block w-3.5 h-3.5 rounded-full bg-primary/10 text-primary text-[8px] leading-[14px] text-center self-start">
                  {task.assignee_id[0]}
                </span>
              )}
            </div>
          </div>
        </div>
        {/* Run/Stop Codex */}
        {!isOverlay && (
          <div className="flex items-center gap-1 mt-2 pt-2 border-t border-border/50">
            {task.assignee_id ? (
              isRunning ? (
                <button
                  onClick={handleStop}
                  disabled={actionLoading !== null}
                  className="flex items-center gap-1 px-2 py-0.5 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {actionLoading === "stop" ? (
                    <Square className="h-3 w-3" />
                  ) : (
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-white animate-pulse" />
                  )}
                  停止
                </button>
              ) : (
                <button
                  onClick={handleRun}
                  disabled={actionLoading !== null}
                  className="flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50"
                >
                  <Play className="h-3 w-3" />
                  运行
                </button>
              )
            ) : (
              <span className="text-xs text-muted-foreground/50" title="请先指派员工">
                <Play className="h-3 w-3 inline mr-0.5" />
                未指派
              </span>
            )}
          </div>
        )}
      </div>
      {!isOverlay && (
        <TaskDetailDialog
          task={task}
          open={showDetail}
          onOpenChange={setShowDetail}
        />
      )}
      {!isOverlay && contextMenu && createPortal(
        <>
          <div
            className="fixed inset-0 z-40"
            onMouseDown={() => setContextMenu(null)}
          />
          <div
            className="fixed z-50 w-44 rounded-lg border border-border bg-popover p-1 shadow-lg"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            role="menu"
            aria-label={`${task.title} 操作菜单`}
            onMouseDown={(e) => e.stopPropagation()}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => void (isRunning ? handleStop() : handleRun())}
              disabled={!task.assignee_id || actionLoading !== null}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
            >
              {isRunning ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />}
              {isRunning ? "停止" : "运行"}
            </button>
            {task.last_codex_session_id && (
              <>
                <div className="my-1 h-px bg-border" />
                <button
                  type="button"
                  role="menuitem"
                  onClick={openContinueDialog}
                  disabled={isRunning || actionLoading !== null}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  <MessageSquarePlus className="h-4 w-4" />
                  继续对话
                </button>
              </>
            )}
            <div className="my-1 h-px bg-border" />
            <button
              type="button"
              role="menuitem"
              onClick={openLogDialog}
              disabled={!task.assignee_id}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
            >
              <ScrollText className="h-4 w-4" />
              查看终端日志
            </button>
            <div className="my-1 h-px bg-border" />
            <button
              type="button"
              role="menuitem"
              onClick={openDeleteDialog}
              disabled={isRunning || deleting}
              title={isRunning ? "运行中的任务不能删除，请先停止" : "删除任务"}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left text-destructive hover:bg-destructive/10 disabled:pointer-events-none disabled:opacity-50"
            >
              <Trash2 className="h-4 w-4" />
              删除
            </button>
          </div>
        </>,
        document.body
      )}
      {!isOverlay && (
        <ContinueConversationDialog
          open={showContinueDialog}
          task={task}
          submitting={actionLoading === "continue"}
          onOpenChange={setShowContinueDialog}
          onConfirm={handleContinueConversation}
        />
      )}
      {!isOverlay && (
        <DeleteTaskDialog
          open={showDeleteDialog}
          task={task}
          deleting={deleting}
          onOpenChange={setShowDeleteDialog}
          onConfirm={handleDelete}
        />
      )}
    </>
  );
}
