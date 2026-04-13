import { create } from "zustand";
import { select } from "@/lib/database";
import type { Task, Subtask, Comment, TaskStatus } from "@/lib/types";
import { onCodexSession, type CodexSession } from "@/lib/codex";
import {
  createComment as createCommentCommand,
  createSubtask as createSubtaskCommand,
  createTask as createTaskCommand,
  deleteSubtask as deleteSubtaskCommand,
  deleteTask as deleteTaskCommand,
  updateSubtaskStatus as updateSubtaskStatusCommand,
  updateTask as updateTaskCommand,
  updateTaskStatus as updateTaskStatusCommand,
} from "@/lib/backend";

function normalizeSubtaskTitle(title: string): string {
  return title.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

interface TaskStore {
  tasks: Task[];
  subtasks: Record<string, Subtask[]>;
  comments: Record<string, Comment[]>;
  loading: boolean;
  fetchTasks: (projectId?: string) => Promise<void>;
  fetchSubtasks: (taskId: string) => Promise<void>;
  fetchComments: (taskId: string) => Promise<void>;
  createTask: (data: { title: string; description?: string; priority?: string; project_id: string; assignee_id?: string }) => Promise<void>;
  updateTaskStatus: (id: string, status: TaskStatus) => Promise<void>;
  updateTask: (id: string, updates: Partial<Pick<Task, "title" | "description" | "priority" | "status" | "assignee_id" | "complexity" | "ai_suggestion" | "last_codex_session_id">>) => Promise<void>;
  deleteTask: (id: string) => Promise<void>;
  addSubtask: (taskId: string, title: string) => Promise<void>;
  addSubtasks: (taskId: string, titles: string[]) => Promise<{ inserted: number; skipped: number }>;
  toggleSubtask: (subtaskId: string, status: string) => Promise<void>;
  deleteSubtask: (subtaskId: string) => Promise<void>;
  addComment: (taskId: string, content: string, employeeId?: string, isAiGenerated?: boolean) => Promise<void>;
  moveTask: (taskId: string, newStatus: TaskStatus) => void;
  setTaskLastSessionId: (taskId: string, sessionId: string) => Promise<void>;
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
  subtasks: {},
  comments: {},
  loading: false,

  fetchTasks: async (projectId) => {
    set({ loading: true });
    try {
      const tasks = projectId
        ? await select<Task>("SELECT * FROM tasks WHERE project_id = $1 ORDER BY updated_at DESC", [projectId])
        : await select<Task>("SELECT * FROM tasks ORDER BY updated_at DESC");
      set({ tasks, loading: false });
    } catch (e) {
      console.error("Failed to fetch tasks:", e);
      set({ loading: false });
    }
  },

  fetchSubtasks: async (taskId) => {
    const subtasks = await select<Subtask>("SELECT * FROM subtasks WHERE task_id = $1 ORDER BY sort_order", [taskId]);
    set((state) => ({ subtasks: { ...state.subtasks, [taskId]: subtasks } }));
  },

  fetchComments: async (taskId) => {
    const comments = await select<Comment>("SELECT * FROM comments WHERE task_id = $1 ORDER BY created_at", [taskId]);
    set((state) => ({ comments: { ...state.comments, [taskId]: comments } }));
  },

  createTask: async (data) => {
    await createTaskCommand({
      ...data,
      description: data.description ?? null,
      assignee_id: data.assignee_id ?? null,
    });
    await get().fetchTasks(data.project_id);
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

  setTaskLastSessionId: async (taskId, sessionId) => {
    set((state) => ({
      tasks: state.tasks.map((task) => (
        task.id === taskId ? { ...task, last_codex_session_id: sessionId } : task
      )),
    }));
    try {
      const task = await updateTaskCommand(taskId, { last_codex_session_id: sessionId });
      set((state) => ({
        tasks: state.tasks.map((current) => (current.id === taskId ? task : current)),
      }));
    } catch (error) {
      console.error("Failed to persist task session id:", error);
    }
  },

  deleteTask: async (id) => {
    await deleteTaskCommand(id);
    await get().fetchTasks();
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
            void get().setTaskLastSessionId(session.task_id, session.session_id);
          }
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
