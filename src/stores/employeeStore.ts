import { create } from "zustand";
import { select } from "@/lib/database";
import {
  createEmployee as createEmployeeCommand,
  deleteEmployee as deleteEmployeeCommand,
  getCodexSessionStatus,
  updateEmployee as updateEmployeeCommand,
  updateEmployeeStatus as updateEmployeeStatusCommand,
} from "@/lib/backend";
import type {
  CodexModelId,
  CodexSessionKind,
  CodexSessionLogLine,
  Employee,
  ReasoningEffort,
} from "@/lib/types";
import { onCodexOutput, onCodexExit, type CodexOutput } from "@/lib/codex";

interface CodexProcessState {
  output: string[];
  running: boolean;
  activeTaskId?: string | null;
}

interface EmployeeStore {
  employees: Employee[];
  loading: boolean;
  codexProcesses: Record<string, CodexProcessState>;
  taskLogs: Record<string, string[]>;
  sessionLogs: Record<string, CodexSessionLogLine[]>;
  fetchEmployees: () => Promise<void>;
  refreshCodexRuntimeStatus: (employeeId: string) => Promise<void>;
  createEmployee: (data: { name: string; role: string; model?: CodexModelId; reasoning_effort?: ReasoningEffort; specialization?: string; system_prompt?: string; project_id?: string }) => Promise<void>;
  updateEmployee: (id: string, updates: Partial<Pick<Employee, "name" | "role" | "model" | "reasoning_effort" | "specialization" | "system_prompt" | "project_id" | "status">>) => Promise<void>;
  deleteEmployee: (id: string) => Promise<void>;
  updateEmployeeStatus: (id: string, status: string) => Promise<void>;
  addCodexOutput: (employeeId: string, line: string, taskId?: string | null, sessionKind?: CodexSessionKind, sessionRecordId?: string | null, sessionEventId?: string | null) => void;
  setCodexRunning: (employeeId: string, running: boolean, activeTaskId?: string | null) => void;
  clearCodexOutput: (employeeId: string) => void;
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

function deriveEmployeeRuntimeStatus(employee: Employee, runtime: Awaited<ReturnType<typeof getCodexSessionStatus>>) {
  if (runtime.running) {
    return "busy";
  }

  if (runtime.session?.status === "failed") {
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

function mergeSessionLogHistory(historyLines: CodexSessionLogLine[], liveLines: CodexSessionLogLine[]) {
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

export const useEmployeeStore = create<EmployeeStore>((set, get) => ({
  employees: [],
  loading: false,
  codexProcesses: {},
  taskLogs: {},
  sessionLogs: {},

  fetchEmployees: async () => {
    set({ loading: true });
    try {
      const employees = await select<Employee>("SELECT * FROM employees ORDER BY created_at");
      const runtimeResults = await Promise.allSettled(
        employees.map(async (employee) => [employee.id, await getCodexSessionStatus(employee.id)] as const)
      );
      const runtimeMap = new Map(
        runtimeResults
          .filter((result): result is PromiseFulfilledResult<readonly [string, Awaited<ReturnType<typeof getCodexSessionStatus>>]> => result.status === "fulfilled")
          .map((result) => result.value)
      );

      set((state) => ({
        employees: employees.map((employee) => {
          const runtime = runtimeMap.get(employee.id);
          return runtime
            ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
            : employee;
        }),
        codexProcesses: employees.reduce<Record<string, CodexProcessState>>((acc, employee) => {
          const runtime = runtimeMap.get(employee.id);
          const previous = state.codexProcesses[employee.id];

          acc[employee.id] = {
            output: previous?.output ?? [],
            running: runtime?.running ?? false,
            activeTaskId: runtime?.running ? runtime.session?.task_id ?? null : null,
          };

          return acc;
        }, { ...state.codexProcesses }),
        loading: false,
      }));
    } catch (e) {
      console.error("Failed to fetch employees:", e);
      set({ loading: false });
    }
  },

  refreshCodexRuntimeStatus: async (employeeId) => {
    try {
      const runtime = await getCodexSessionStatus(employeeId);
      set((state) => ({
        employees: state.employees.map((employee) => (
          employee.id === employeeId
            ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
            : employee
        )),
        codexProcesses: {
          ...state.codexProcesses,
          [employeeId]: {
            output: state.codexProcesses[employeeId]?.output ?? [],
            running: runtime.running,
            activeTaskId: runtime.running ? runtime.session?.task_id ?? null : null,
          },
        },
      }));
    } catch (error) {
      console.error(`Failed to refresh codex runtime status for ${employeeId}:`, error);
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
    set((state) => {
      const { [id]: _, ...rest } = state.codexProcesses;
      return { codexProcesses: rest };
    });
    await get().fetchEmployees();
  },

  updateEmployeeStatus: async (id, status) => {
    const employee = await updateEmployeeStatusCommand(id, status);
    set((state) => ({
      employees: state.employees.map((e) => (e.id === id ? employee : e)),
    }));
  },

  addCodexOutput: (employeeId, line, taskId, sessionKind = "execution", sessionRecordId, sessionEventId) => {
    set((state) => ({
      codexProcesses: {
        ...state.codexProcesses,
        [employeeId]: {
          ...state.codexProcesses[employeeId],
          output: [...(state.codexProcesses[employeeId]?.output ?? []).slice(-199), line],
          running: state.codexProcesses[employeeId]?.running ?? false,
        },
      },
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
    }));
  },

  setCodexRunning: (employeeId, running, activeTaskId) => {
    set((state) => ({
      codexProcesses: {
        ...state.codexProcesses,
        [employeeId]: {
          ...state.codexProcesses[employeeId],
          running,
          activeTaskId: running
            ? activeTaskId ?? state.codexProcesses[employeeId]?.activeTaskId ?? null
            : null,
        },
      },
    }));
  },

  clearCodexOutput: (employeeId) => {
    set((state) => ({
      codexProcesses: {
        ...state.codexProcesses,
        [employeeId]: {
          ...state.codexProcesses[employeeId],
          output: [],
        },
      },
    }));
  },

  clearTaskCodexOutput: (taskId, sessionKind = "execution") => {
    set((state) => ({
      taskLogs: {
        ...state.taskLogs,
        [buildTaskLogKey(taskId, sessionKind)]: [],
      },
    }));
  },

  hydrateSessionLog: (sessionRecordId, lines) => {
    set((state) => ({
      sessionLogs: {
        ...state.sessionLogs,
        [sessionRecordId]: mergeSessionLogHistory(
          lines,
          state.sessionLogs[sessionRecordId] ?? [],
        ),
      },
    }));
  },

  clearSessionCodexOutput: (sessionRecordId) => {
    set((state) => ({
      sessionLogs: {
        ...state.sessionLogs,
        [sessionRecordId]: [],
      },
    }));
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
          get().setCodexRunning(exit.employee_id, false, null);
          void get().updateEmployeeStatus(exit.employee_id, "offline");
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
