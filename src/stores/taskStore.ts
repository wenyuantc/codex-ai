import { create } from "zustand";
import { select } from "@/lib/database";
import type { CodexSessionKind, Task, TaskAttachment, Subtask, Comment, TaskStatus } from "@/lib/types";
import { onCodexExit, onCodexSession, type CodexSession } from "@/lib/codex";
import {
  addTaskAttachments as addTaskAttachmentsCommand,
  createComment as createCommentCommand,
  createSubtask as createSubtaskCommand,
  createTask as createTaskCommand,
  deleteSubtask as deleteSubtaskCommand,
  deleteTaskAttachment as deleteTaskAttachmentCommand,
  deleteTask as deleteTaskCommand,
  getTaskAutomationState as getTaskAutomationStateCommand,
  restartTaskAutomation as restartTaskAutomationCommand,
  setTaskAutomationMode as setTaskAutomationModeCommand,
  updateSubtaskStatus as updateSubtaskStatusCommand,
  updateTask as updateTaskCommand,
  updateTaskStatus as updateTaskStatusCommand,
} from "@/lib/backend";
import type { TaskAutomationMode, TaskAutomationState } from "@/lib/types";
import { useProjectStore } from "./projectStore";

function normalizeSubtaskTitle(title: string): string {
  return title.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

interface TaskStore {
  tasks: Task[];
  attachments: Record<string, TaskAttachment[]>;
  subtasks: Record<string, Subtask[]>;
  comments: Record<string, Comment[]>;
  automationStates: Record<string, TaskAutomationState | null>;
  activeProjectId?: string;
  loading: boolean;
  fetchTasks: (projectId?: string) => Promise<void>;
  fetchAttachments: (taskId: string) => Promise<void>;
  fetchSubtasks: (taskId: string) => Promise<void>;
  fetchComments: (taskId: string) => Promise<void>;
  createTask: (
    data: {
      title: string;
      description?: string;
      priority?: string;
      project_id: string;
      use_worktree?: boolean;
      assignee_id?: string;
      reviewer_id?: string;
      attachment_source_paths?: string[];
    },
    options?: { refreshProjectId?: string },
  ) => Promise<Task>;
  updateTaskStatus: (id: string, status: TaskStatus) => Promise<void>;
  updateTask: (id: string, updates: Partial<Pick<Task, "title" | "description" | "priority" | "status" | "assignee_id" | "reviewer_id" | "complexity" | "ai_suggestion" | "last_codex_session_id" | "last_review_session_id">>) => Promise<void>;
  setTaskAutomationMode: (taskId: string, automationMode: TaskAutomationMode | null) => Promise<void>;
  fetchTaskAutomationState: (taskId: string) => Promise<void>;
  restartTaskAutomation: (taskId: string) => Promise<void>;
  deleteTask: (id: string) => Promise<void>;
  addTaskAttachments: (taskId: string, sourcePaths: string[]) => Promise<void>;
  deleteTaskAttachment: (taskId: string, attachmentId: string) => Promise<void>;
  addSubtask: (taskId: string, title: string) => Promise<void>;
  addSubtasks: (taskId: string, titles: string[]) => Promise<{ inserted: number; skipped: number }>;
  toggleSubtask: (subtaskId: string, status: string) => Promise<void>;
  deleteSubtask: (subtaskId: string) => Promise<void>;
  addComment: (taskId: string, content: string, employeeId?: string, isAiGenerated?: boolean) => Promise<void>;
  moveTask: (taskId: string, newStatus: TaskStatus) => void;
  setTaskLastSessionId: (taskId: string, sessionId: string, sessionKind: CodexSessionKind) => Promise<void>;
  initCodexSessionListeners: () => () => void;
}

let codexSessionListenerRefCount = 0;
let codexSessionListenersInitPromise: Promise<void> | null = null;
let codexSessionListenersCleanup: (() => void) | null = null;

function releaseCodexSessionListeners() {
  codexSessionListenersCleanup?.();
  codexSessionListenersCleanup = null;
  codexSessionListenersInitPromise = null;
}

export const useTaskStore = create<TaskStore>((set, get) => ({
  tasks: [],
  attachments: {},
  subtasks: {},
  comments: {},
  automationStates: {},
  activeProjectId: undefined,
  loading: false,

  fetchTasks: async (projectId) => {
    set({ loading: true, activeProjectId: projectId });
    try {
      const rawTasks = projectId
        ? await select<Task>("SELECT * FROM tasks WHERE project_id = $1 ORDER BY updated_at DESC", [projectId])
        : await select<Task>("SELECT * FROM tasks ORDER BY updated_at DESC");
      const visibleProjectIds = new Set(useProjectStore.getState().projects.map((project) => project.id));
      const tasks = rawTasks.filter((task) => visibleProjectIds.has(task.project_id));
      const automationEntries = await Promise.all(
        tasks
          .filter((task) => task.automation_mode === "review_fix_loop_v1")
          .map(async (task) => {
            try {
              const automationState = await getTaskAutomationStateCommand(task.id);
              return [task.id, automationState] as const;
            } catch (error) {
              console.error(`Failed to fetch automation state for task ${task.id}:`, error);
              return [task.id, null] as const;
            }
          }),
      );

      set((state) => ({
        tasks,
        loading: false,
        activeProjectId: projectId,
        automationStates: {
          ...state.automationStates,
          ...Object.fromEntries(automationEntries),
        },
      }));
    } catch (e) {
      console.error("Failed to fetch tasks:", e);
      set({ loading: false });
    }
  },

  fetchAttachments: async (taskId) => {
    const attachments = await select<TaskAttachment>(
      "SELECT * FROM task_attachments WHERE task_id = $1 ORDER BY sort_order, created_at",
      [taskId],
    );
    set((state) => ({ attachments: { ...state.attachments, [taskId]: attachments } }));
  },

  fetchSubtasks: async (taskId) => {
    const subtasks = await select<Subtask>("SELECT * FROM subtasks WHERE task_id = $1 ORDER BY sort_order", [taskId]);
    set((state) => ({ subtasks: { ...state.subtasks, [taskId]: subtasks } }));
  },

  fetchComments: async (taskId) => {
    const comments = await select<Comment>("SELECT * FROM comments WHERE task_id = $1 ORDER BY created_at", [taskId]);
    set((state) => ({ comments: { ...state.comments, [taskId]: comments } }));
  },

  createTask: async (data, options) => {
    const task = await createTaskCommand({
      ...data,
      description: data.description ?? null,
      use_worktree: data.use_worktree ?? false,
      assignee_id: data.assignee_id ?? null,
      reviewer_id: data.reviewer_id ?? null,
      attachment_source_paths: data.attachment_source_paths ?? [],
    });
    await get().fetchTasks(options?.refreshProjectId ?? get().activeProjectId);
    return task;
  },

  updateTaskStatus: async (id, status) => {
    const task = await updateTaskStatusCommand(id, status);
    set((state) => ({
      tasks: state.tasks.map((current) => (current.id === id ? task : current)),
    }));
  },

  updateTask: async (id, updates) => {
    const task = await updateTaskCommand(id, updates);
    set((state) => ({
      tasks: state.tasks.map((current) => (current.id === id ? task : current)),
    }));
  },

  setTaskAutomationMode: async (taskId, automationMode) => {
    const task = await setTaskAutomationModeCommand({
      task_id: taskId,
      automation_mode: automationMode,
    });
    set((state) => ({
      tasks: state.tasks.map((current) => (current.id === taskId ? task : current)),
    }));
    await get().fetchTaskAutomationState(taskId);
  },

  fetchTaskAutomationState: async (taskId) => {
    const state = await getTaskAutomationStateCommand(taskId);
    set((current) => ({
      automationStates: {
        ...current.automationStates,
        [taskId]: state,
      },
    }));
  },

  restartTaskAutomation: async (taskId) => {
    await restartTaskAutomationCommand(taskId);
    await get().fetchTaskAutomationState(taskId);
    await get().fetchTasks(get().activeProjectId);
  },

  setTaskLastSessionId: async (taskId, sessionId, sessionKind) => {
    set((state) => ({
      tasks: state.tasks.map((task) => (
        task.id === taskId
          ? sessionKind === "review"
            ? { ...task, last_review_session_id: sessionId }
            : { ...task, last_codex_session_id: sessionId }
          : task
      )),
    }));
    try {
      const task = await updateTaskCommand(
        taskId,
        sessionKind === "review"
          ? { last_review_session_id: sessionId }
          : { last_codex_session_id: sessionId },
      );
      set((state) => ({
        tasks: state.tasks.map((current) => (current.id === taskId ? task : current)),
      }));
    } catch (error) {
      console.error("Failed to persist task session id:", error);
    }
  },

  deleteTask: async (id) => {
    await deleteTaskCommand(id);
    set((state) => {
      const { [id]: _attachments, ...attachments } = state.attachments;
      const { [id]: _subtasks, ...subtasks } = state.subtasks;
      const { [id]: _comments, ...comments } = state.comments;
      const { [id]: _automationState, ...automationStates } = state.automationStates;
      return { attachments, subtasks, comments, automationStates };
    });
    await get().fetchTasks(get().activeProjectId);
  },

  addTaskAttachments: async (taskId, sourcePaths) => {
    if (sourcePaths.length === 0) return;
    const attachments = await addTaskAttachmentsCommand(taskId, sourcePaths);
    set((state) => ({
      attachments: {
        ...state.attachments,
        [taskId]: attachments,
      },
    }));
  },

  deleteTaskAttachment: async (taskId, attachmentId) => {
    await deleteTaskAttachmentCommand(attachmentId);
    await get().fetchAttachments(taskId);
  },

  addSubtask: async (taskId, title) => {
    await createSubtaskCommand(taskId, title);
    await get().fetchSubtasks(taskId);
  },

  addSubtasks: async (taskId, titles) => {
    await get().fetchSubtasks(taskId);

    const currentSubtasks = get().subtasks[taskId] ?? [];
    const existingTitles = new Set(currentSubtasks.map((subtask) => normalizeSubtaskTitle(subtask.title)));
    let inserted = 0;
    let skipped = 0;

    for (const rawTitle of titles) {
      const title = rawTitle.trim().replace(/\s+/g, " ");
      const normalizedTitle = normalizeSubtaskTitle(title);

      if (!normalizedTitle || existingTitles.has(normalizedTitle)) {
        skipped += 1;
        continue;
      }

      await createSubtaskCommand(taskId, title);
      existingTitles.add(normalizedTitle);
      inserted += 1;
    }

    await get().fetchSubtasks(taskId);
    return { inserted, skipped };
  },

  toggleSubtask: async (subtaskId, status) => {
    await updateSubtaskStatusCommand(subtaskId, status);
    const entries = Object.entries(get().subtasks);
    for (const [taskId, subs] of entries) {
      if (subs.some((s) => s.id === subtaskId)) {
        await get().fetchSubtasks(taskId);
        break;
      }
    }
  },

  deleteSubtask: async (subtaskId) => {
    const entries = Object.entries(get().subtasks);
    let taskId = "";
    for (const [tid, subs] of entries) {
      if (subs.some((s) => s.id === subtaskId)) {
        taskId = tid;
        break;
      }
    }
    await deleteSubtaskCommand(subtaskId);
    if (taskId) await get().fetchSubtasks(taskId);
  },

  addComment: async (taskId, content, employeeId, isAiGenerated = false) => {
    await createCommentCommand(taskId, content, employeeId ?? null, isAiGenerated);
    await get().fetchComments(taskId);
  },

  moveTask: (taskId, newStatus) => {
    set((state) => ({
      tasks: state.tasks.map((t) => (t.id === taskId ? { ...t, status: newStatus } : t)),
    }));
  },

  initCodexSessionListeners: () => {
    codexSessionListenerRefCount += 1;

    if (!codexSessionListenersInitPromise && !codexSessionListenersCleanup) {
      codexSessionListenersInitPromise = Promise.all([
        onCodexSession((session: CodexSession) => {
          if (session.task_id) {
            void get().setTaskLastSessionId(
              session.task_id,
              session.session_id,
              session.session_kind,
            );
          }
        }),
        onCodexExit((exit) => {
          if (!exit.task_id) {
            return;
          }

          void get().fetchTaskAutomationState(exit.task_id);
          void get().fetchTasks(get().activeProjectId);
        }),
      ])
        .then((unlisteners) => {
          codexSessionListenersCleanup = () => {
            unlisteners.forEach((unlisten) => unlisten());
          };
          codexSessionListenersInitPromise = null;

          if (codexSessionListenerRefCount === 0) {
            releaseCodexSessionListeners();
          }
        })
        .catch((error) => {
          console.error("Failed to initialize Codex session listeners:", error);
          codexSessionListenersInitPromise = null;
          codexSessionListenersCleanup = null;
        });
    }

    let released = false;

    return () => {
      if (released) return;
      released = true;
      codexSessionListenerRefCount = Math.max(0, codexSessionListenerRefCount - 1);

      if (codexSessionListenerRefCount === 0 && codexSessionListenersCleanup) {
        releaseCodexSessionListeners();
      }
    };
  },
}));
