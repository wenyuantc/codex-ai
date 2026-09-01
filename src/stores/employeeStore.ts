import { create } from "zustand";

import {
  createEmployee as createEmployeeCommand,
  deleteEmployee as deleteEmployeeCommand,
  getEmployeeRuntimeStatus,
  listEmployees as listEmployeesCommand,
  updateEmployee as updateEmployeeCommand,
  updateEmployeeStatus as updateEmployeeStatusCommand,
} from "@/lib/backend";
import {
  onCodexExit,
  onCodexOutput,
  onCodexSession,
  type CodexOutput,
  type CodexSession,
} from "@/lib/codex";
import {
  onClaudeExit,
  onClaudeOutput,
  onClaudeSession,
  type ClaudeOutput,
  type ClaudeSession,
} from "@/lib/claude";
import {
  onGrokExit,
  onGrokOutput,
  onGrokSession,
  type GrokOutput,
  type GrokSession,
} from "@/lib/grok";
import {
  onNativeExit,
  onNativeOutput,
  onNativeSession,
  onNativeTextDelta,
  type NativeExit,
  type NativeOutput,
  type NativeSession,
  type NativeTextDelta,
} from "@/lib/native";
import {
  onOpenCodeExit,
  onOpenCodeOutput,
  onOpenCodeSession,
  type OpenCodeOutputEvent as OpenCodeOutput,
  type OpenCodeSessionEvent as OpenCodeSession,
} from "@/lib/opencode";
import {
  applyTodoSnapshotToKeys,
  extractLatestSessionTodos,
  parseSessionTodoSnapshot,
  type SessionTodoItem,
} from "@/lib/sessionTodos";
import type {
  AiProvider,
  CodexSessionKind,
  CodexSessionLogLine,
  Employee,
  EmployeeRuntimeStatus,
} from "@/lib/types";

/** Fragments of an answer still being generated, keyed the same way as
 * `taskLogs` / `sessionLogs`. Live only — never persisted or exported. */
export interface StreamingText {
  reasoning: string;
  text: string;
}

interface EmployeeStore {
  employees: Employee[];
  loading: boolean;
  employeeRuntime: Record<string, EmployeeRuntimeStatus>;
  taskLogs: Record<string, string[]>;
  sessionLogs: Record<string, CodexSessionLogLine[]>;
  sessionTodos: Record<string, SessionTodoItem[]>;
  streamingTexts: Record<string, StreamingText>;
  fetchEmployees: () => Promise<void>;
  refreshEmployeeRuntimeStatus: (employeeId: string) => Promise<EmployeeRuntimeStatus | null>;
  createEmployee: (data: {
    name: string;
    role: string;
    model?: string;
    reasoning_effort?: string;
    specialization?: string;
    system_prompt?: string;
    project_id?: string;
    ai_provider?: AiProvider;
    ai_channel_id?: string | null;
  }) => Promise<void>;
  updateEmployee: (
    id: string,
    updates: Partial<
      Pick<
        Employee,
        | "name"
        | "role"
        | "model"
        | "reasoning_effort"
        | "specialization"
        | "system_prompt"
        | "project_id"
        | "status"
        | "ai_provider"
        | "ai_channel_id"
      >
    >,
  ) => Promise<void>;
  deleteEmployee: (id: string) => Promise<void>;
  updateEmployeeStatus: (id: string, status: string) => Promise<void>;
  addCodexOutput: (
    employeeId: string,
    line: string,
    taskId?: string | null,
    sessionKind?: CodexSessionKind,
    sessionRecordId?: string | null,
    sessionEventId?: string | null,
  ) => void;
  applyNativeTextDelta: (delta: NativeTextDelta) => void;
  clearTaskCodexOutput: (taskId: string, sessionKind?: CodexSessionKind) => void;
  hydrateSessionLog: (sessionRecordId: string, lines: CodexSessionLogLine[]) => void;
  clearSessionCodexOutput: (sessionRecordId: string) => void;
  initCodexListeners: () => () => void;
}

let codexListenerRefCount = 0;
let codexListenersInitPromise: Promise<void> | null = null;
let codexListenersCleanup: (() => void) | null = null;

function releaseCodexListeners() {
  codexListenersCleanup?.();
  codexListenersCleanup = null;
  codexListenersInitPromise = null;
}

function deriveEmployeeRuntimeStatus(employee: Employee, runtime: EmployeeRuntimeStatus) {
  if (runtime.running) {
    return "busy";
  }

  if (runtime.latest_session?.status === "failed") {
    return "error";
  }

  if (employee.status === "busy" || employee.status === "online") {
    return "offline";
  }

  return employee.status;
}

export function buildTaskLogKey(taskId: string, sessionKind: CodexSessionKind = "execution") {
  return `${taskId}::${sessionKind}`;
}

function todoCacheKeys(
  taskId?: string | null,
  sessionKind: CodexSessionKind = "execution",
  sessionRecordId?: string | null,
): string[] {
  const keys: string[] = [];
  if (sessionRecordId) {
    keys.push(sessionRecordId);
  }
  if (taskId) {
    keys.push(buildTaskLogKey(taskId, sessionKind));
  }
  return keys;
}

const EMPTY_STREAMING_TEXT: StreamingText = { reasoning: "", text: "" };

/** A session is watched either by its record id or, for a task terminal, by
 * its task log key, so a delta has to land under both. */
export function streamingTextKeys(
  delta: Pick<NativeTextDelta, "session_record_id" | "task_id" | "session_kind">,
): string[] {
  const keys = [delta.session_record_id];
  if (delta.task_id) {
    keys.push(buildTaskLogKey(delta.task_id, delta.session_kind));
  }
  return keys;
}

export function applyStreamingDelta(
  streamingTexts: Record<string, StreamingText>,
  delta: NativeTextDelta,
): Record<string, StreamingText> {
  const keys = streamingTextKeys(delta);
  if (delta.clear) {
    if (!keys.some((key) => key in streamingTexts)) {
      return streamingTexts;
    }
    const next = { ...streamingTexts };
    for (const key of keys) {
      delete next[key];
    }
    return next;
  }
  if (!delta.delta) {
    return streamingTexts;
  }
  const next = { ...streamingTexts };
  for (const key of keys) {
    const current = next[key] ?? EMPTY_STREAMING_TEXT;
    next[key] =
      delta.segment === "reasoning"
        ? { ...current, reasoning: current.reasoning + delta.delta }
        : { ...current, text: current.text + delta.delta };
  }
  return next;
}

export function applyEmployeeDeleted(
  state: {
    employees: Employee[];
    employeeRuntime: Record<string, EmployeeRuntimeStatus>;
  },
  id: string,
): {
  employees: Employee[];
  employeeRuntime: Record<string, EmployeeRuntimeStatus>;
} {
  const { [id]: _removed, ...employeeRuntime } = state.employeeRuntime;
  return {
    employees: state.employees.filter((employee) => employee.id !== id),
    employeeRuntime,
  };
}

let syntheticSessionLogEventCounter = 0;

function nextSyntheticSessionLogEventId() {
  syntheticSessionLogEventCounter += 1;
  return `live:${syntheticSessionLogEventCounter}`;
}

function appendSessionLogLine(
  existingLines: CodexSessionLogLine[],
  line: string,
  sessionEventId?: string | null,
) {
  if (sessionEventId && existingLines.some((entry) => entry.event_id === sessionEventId)) {
    return existingLines;
  }

  return [
    ...existingLines.slice(-1999),
    {
      event_id: sessionEventId ?? nextSyntheticSessionLogEventId(),
      line,
    },
  ];
}

function mergeSessionLogHistory(
  historyLines: CodexSessionLogLine[],
  liveLines: CodexSessionLogLine[],
) {
  const mergedLines = historyLines.slice(-2000);
  const seenEventIds = new Set(mergedLines.map((entry) => entry.event_id));

  for (const liveLine of liveLines) {
    if (!seenEventIds.has(liveLine.event_id)) {
      seenEventIds.add(liveLine.event_id);
      mergedLines.push(liveLine);
    }
  }

  return mergedLines.slice(-2000);
}

async function syncEmployeeRuntime(employeeId: string) {
  const runtime = await getEmployeeRuntimeStatus(employeeId);
  useEmployeeStore.setState((state) => ({
    employees: state.employees.map((employee) =>
      employee.id === employeeId
        ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
        : employee,
    ),
    employeeRuntime: {
      ...state.employeeRuntime,
      [employeeId]: runtime,
    },
  }));
  return runtime;
}

export const useEmployeeStore = create<EmployeeStore>((set, get) => ({
  employees: [],
  loading: false,
  employeeRuntime: {},
  taskLogs: {},
  sessionLogs: {},
  sessionTodos: {},
  streamingTexts: {},

  fetchEmployees: async () => {
    set({ loading: true });
    try {
      const employees = await listEmployeesCommand();
      const runtimeResults = await Promise.allSettled(
        employees.map(
          async (employee) => [employee.id, await getEmployeeRuntimeStatus(employee.id)] as const,
        ),
      );
      const runtimeMap = new Map(
        runtimeResults
          .filter(
            (result): result is PromiseFulfilledResult<readonly [string, EmployeeRuntimeStatus]> =>
              result.status === "fulfilled",
          )
          .map((result) => result.value),
      );

      set((state) => ({
        employees: employees.map((employee) => {
          const runtime = runtimeMap.get(employee.id);
          return runtime
            ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
            : employee;
        }),
        employeeRuntime: employees.reduce<Record<string, EmployeeRuntimeStatus>>(
          (acc, employee) => {
            const runtime = runtimeMap.get(employee.id);
            if (runtime) {
              acc[employee.id] = runtime;
            } else if (state.employeeRuntime[employee.id]) {
              acc[employee.id] = state.employeeRuntime[employee.id];
            }
            return acc;
          },
          { ...state.employeeRuntime },
        ),
        loading: false,
      }));
    } catch (error) {
      console.error("Failed to fetch employees:", error);
      set({ loading: false });
    }
  },

  refreshEmployeeRuntimeStatus: async (employeeId) => {
    try {
      return await syncEmployeeRuntime(employeeId);
    } catch (error) {
      console.error(`Failed to refresh runtime status for ${employeeId}:`, error);
      return null;
    }
  },

  createEmployee: async (data) => {
    await createEmployeeCommand({
      ...data,
      specialization: data.specialization ?? null,
      system_prompt: data.system_prompt ?? null,
      project_id: data.project_id ?? null,
    });
    await get().fetchEmployees();
  },

  updateEmployee: async (id, updates) => {
    await updateEmployeeCommand(id, updates);
    await get().fetchEmployees();
  },

  deleteEmployee: async (id) => {
    await deleteEmployeeCommand(id);
    set((state) => applyEmployeeDeleted(state, id));
    await get().fetchEmployees();
  },

  updateEmployeeStatus: async (id, status) => {
    const employee = await updateEmployeeStatusCommand(id, status);
    set((state) => ({
      employees: state.employees.map((current) =>
        current.id === id
          ? {
              ...employee,
              status:
                status === "busy" || status === "online"
                  ? status
                  : state.employeeRuntime[id]
                    ? deriveEmployeeRuntimeStatus(employee, state.employeeRuntime[id])
                    : employee.status,
            }
          : current,
      ),
    }));
  },

  addCodexOutput: (
    _employeeId,
    line,
    taskId,
    sessionKind = "execution",
    sessionRecordId,
    sessionEventId,
  ) => {
    set((state) => ({
      taskLogs: taskId
        ? {
            ...state.taskLogs,
            [buildTaskLogKey(taskId, sessionKind)]: [
              ...(state.taskLogs[buildTaskLogKey(taskId, sessionKind)] ?? []).slice(-199),
              line,
            ],
          }
        : state.taskLogs,
      sessionLogs: sessionRecordId
        ? {
            ...state.sessionLogs,
            [sessionRecordId]: appendSessionLogLine(
              state.sessionLogs[sessionRecordId] ?? [],
              line,
              sessionEventId,
            ),
          }
        : state.sessionLogs,
      sessionTodos: applyTodoSnapshotToKeys(
        state.sessionTodos,
        todoCacheKeys(taskId, sessionKind, sessionRecordId),
        parseSessionTodoSnapshot(line),
      ),
    }));
  },

  applyNativeTextDelta: (delta) => {
    set((state) => {
      const streamingTexts = applyStreamingDelta(state.streamingTexts, delta);
      return streamingTexts === state.streamingTexts ? {} : { streamingTexts };
    });
  },

  clearTaskCodexOutput: (taskId, sessionKind = "execution") => {
    const key = buildTaskLogKey(taskId, sessionKind);
    set((state) => {
      const { [key]: _cleared, ...streamingTexts } = state.streamingTexts;
      const { [key]: _clearedTodos, ...sessionTodos } = state.sessionTodos;
      return {
        taskLogs: {
          ...state.taskLogs,
          [key]: [],
        },
        sessionTodos,
        streamingTexts,
      };
    });
  },

  hydrateSessionLog: (sessionRecordId, lines) => {
    set((state) => {
      const merged = mergeSessionLogHistory(lines, state.sessionLogs[sessionRecordId] ?? []);
      const snapshot = extractLatestSessionTodos(merged.map((entry) => entry.line));
      return {
        sessionLogs: {
          ...state.sessionLogs,
          [sessionRecordId]: merged,
        },
        sessionTodos: applyTodoSnapshotToKeys(state.sessionTodos, [sessionRecordId], snapshot),
      };
    });
  },

  clearSessionCodexOutput: (sessionRecordId) => {
    set((state) => {
      const { [sessionRecordId]: _cleared, ...streamingTexts } = state.streamingTexts;
      const { [sessionRecordId]: _clearedTodos, ...sessionTodos } = state.sessionTodos;
      return {
        sessionLogs: {
          ...state.sessionLogs,
          [sessionRecordId]: [],
        },
        sessionTodos,
        streamingTexts,
      };
    });
  },

  initCodexListeners: () => {
    codexListenerRefCount += 1;

    if (!codexListenersInitPromise && !codexListenersCleanup) {
      codexListenersInitPromise = Promise.all([
        onCodexOutput((output: CodexOutput) => {
          get().addCodexOutput(
            output.employee_id,
            output.line,
            output.task_id,
            output.session_kind,
            output.session_record_id,
            output.session_event_id,
          );
        }),
        onCodexSession((session: CodexSession) => {
          set((state) => ({
            employees: state.employees.map((employee) =>
              employee.id === session.employee_id ? { ...employee, status: "busy" } : employee,
            ),
          }));
          void get().refreshEmployeeRuntimeStatus(session.employee_id);
        }),
        onCodexExit((exit) => {
          if (exit.line) {
            get().addCodexOutput(
              exit.employee_id,
              exit.line,
              exit.task_id,
              exit.session_kind,
              exit.session_record_id,
              exit.session_event_id,
            );
          }

          void (async () => {
            const runtime = await syncEmployeeRuntime(exit.employee_id).catch((error) => {
              console.error(`Failed to sync runtime after exit for ${exit.employee_id}:`, error);
              return null;
            });

            if (!runtime?.running) {
              void get().updateEmployeeStatus(
                exit.employee_id,
                exit.code === 0 ? "offline" : "error",
              );
            }
          })();
        }),
        onClaudeOutput((output: ClaudeOutput) => {
          get().addCodexOutput(
            output.employee_id,
            output.line,
            output.task_id,
            output.session_kind,
            output.session_record_id,
            output.session_event_id,
          );
        }),
        onClaudeSession((session: ClaudeSession) => {
          set((state) => ({
            employees: state.employees.map((employee) =>
              employee.id === session.employee_id ? { ...employee, status: "busy" } : employee,
            ),
          }));
          void get().refreshEmployeeRuntimeStatus(session.employee_id);
        }),
        onClaudeExit((exit) => {
          if (exit.line) {
            get().addCodexOutput(
              exit.employee_id,
              exit.line,
              exit.task_id,
              exit.session_kind,
              exit.session_record_id,
              exit.session_event_id,
            );
          }

          void (async () => {
            const runtime = await syncEmployeeRuntime(exit.employee_id).catch((error) => {
              console.error(
                `Failed to sync runtime after Claude exit for ${exit.employee_id}:`,
                error,
              );
              return null;
            });

            if (!runtime?.running) {
              void get().updateEmployeeStatus(
                exit.employee_id,
                exit.status === "exited" ? "offline" : "error",
              );
            }
          })();
        }),
        onOpenCodeOutput((output: OpenCodeOutput) => {
          get().addCodexOutput(
            output.employee_id,
            output.line,
            output.task_id,
            output.session_kind as CodexSessionKind,
            output.session_record_id,
            output.session_event_id,
          );
        }),
        onOpenCodeSession((session: OpenCodeSession) => {
          set((state) => ({
            employees: state.employees.map((employee) =>
              employee.id === session.employee_id ? { ...employee, status: "busy" } : employee,
            ),
          }));
          void get().refreshEmployeeRuntimeStatus(session.employee_id);
        }),
        onOpenCodeExit((exit) => {
          if (exit.line) {
            get().addCodexOutput(
              exit.employee_id,
              exit.line,
              exit.task_id,
              exit.session_kind as CodexSessionKind,
              exit.session_record_id,
              exit.session_event_id,
            );
          }

          void (async () => {
            const runtime = await syncEmployeeRuntime(exit.employee_id).catch((error) => {
              console.error(
                `Failed to sync runtime after OpenCode exit for ${exit.employee_id}:`,
                error,
              );
              return null;
            });

            if (!runtime?.running) {
              void get().updateEmployeeStatus(
                exit.employee_id,
                exit.code === 0 ? "offline" : "error",
              );
            }
          })();
        }),
        onGrokOutput((output: GrokOutput) => {
          get().addCodexOutput(
            output.employee_id,
            output.line,
            output.task_id,
            output.session_kind,
            output.session_record_id,
            output.session_event_id,
          );
        }),
        onGrokSession((session: GrokSession) => {
          set((state) => ({
            employees: state.employees.map((employee) =>
              employee.id === session.employee_id ? { ...employee, status: "busy" } : employee,
            ),
          }));
          void get().refreshEmployeeRuntimeStatus(session.employee_id);
        }),
        onGrokExit((exit) => {
          if (exit.line) {
            get().addCodexOutput(
              exit.employee_id,
              exit.line,
              exit.task_id,
              exit.session_kind,
              exit.session_record_id,
              exit.session_event_id,
            );
          }

          void (async () => {
            const runtime = await syncEmployeeRuntime(exit.employee_id).catch((error) => {
              console.error(
                `Failed to sync runtime after Grok exit for ${exit.employee_id}:`,
                error,
              );
              return null;
            });

            if (!runtime?.running) {
              void get().updateEmployeeStatus(
                exit.employee_id,
                exit.status === "exited" ? "offline" : "error",
              );
            }
          })();
        }),
        onNativeOutput((output: NativeOutput) => {
          get().addCodexOutput(
            output.employee_id,
            output.line,
            output.task_id,
            output.session_kind,
            output.session_record_id,
            output.session_event_id,
          );
        }),
        onNativeTextDelta((delta: NativeTextDelta) => {
          get().applyNativeTextDelta(delta);
        }),
        onNativeSession((session: NativeSession) => {
          set((state) => ({
            employees: state.employees.map((employee) =>
              employee.id === session.employee_id ? { ...employee, status: "busy" } : employee,
            ),
          }));
          void get().refreshEmployeeRuntimeStatus(session.employee_id);
        }),
        onNativeExit((exit: NativeExit) => {
          get().applyNativeTextDelta({
            employee_id: exit.employee_id,
            task_id: exit.task_id,
            session_kind: exit.session_kind,
            session_record_id: exit.session_record_id,
            segment: "text",
            delta: "",
            clear: true,
          });
          if (exit.line) {
            get().addCodexOutput(
              exit.employee_id,
              exit.line,
              exit.task_id,
              exit.session_kind,
              exit.session_record_id,
              exit.session_event_id,
            );
          }

          void (async () => {
            const runtime = await syncEmployeeRuntime(exit.employee_id).catch((error) => {
              console.error(
                `Failed to sync runtime after native exit for ${exit.employee_id}:`,
                error,
              );
              return null;
            });

            if (!runtime?.running) {
              void get().updateEmployeeStatus(
                exit.employee_id,
                exit.code === 0 ? "offline" : "error",
              );
            }
          })();
        }),
      ])
        .then((unlisteners) => {
          codexListenersCleanup = () => {
            unlisteners.forEach((unlisten) => unlisten());
          };
          codexListenersInitPromise = null;

          if (codexListenerRefCount === 0) {
            releaseCodexListeners();
          }
        })
        .catch((error) => {
          console.error("Failed to initialize Codex listeners:", error);
          codexListenersInitPromise = null;
          codexListenersCleanup = null;
        });
    }

    let released = false;

    return () => {
      if (released) return;
      released = true;
      codexListenerRefCount = Math.max(0, codexListenerRefCount - 1);

      if (codexListenerRefCount === 0 && codexListenersCleanup) {
        releaseCodexListeners();
      }
    };
  },
}));
