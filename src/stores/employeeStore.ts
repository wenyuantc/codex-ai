import { create } from "zustand";

import { select } from "@/lib/database";
import {
  createEmployee as createEmployeeCommand,
  deleteEmployee as deleteEmployeeCommand,
  getEmployeeRuntimeStatus,
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
import type {
  CodexModelId,
  CodexSessionKind,
  CodexSessionLogLine,
  Employee,
  EmployeeRuntimeStatus,
  ReasoningEffort,
} from "@/lib/types";

interface EmployeeStore {
  employees: Employee[];
  loading: boolean;
  employeeRuntime: Record<string, EmployeeRuntimeStatus>;
  taskLogs: Record<string, string[]>;
  sessionLogs: Record<string, CodexSessionLogLine[]>;
  fetchEmployees: () => Promise<void>;
  refreshEmployeeRuntimeStatus: (employeeId: string) => Promise<EmployeeRuntimeStatus | null>;
  createEmployee: (data: {
    name: string;
    role: string;
    model?: CodexModelId;
    reasoning_effort?: ReasoningEffort;
    specialization?: string;
    system_prompt?: string;
    project_id?: string;
  }) => Promise<void>;
  updateEmployee: (
    id: string,
    updates: Partial<
      Pick<
        Employee,
        "name" | "role" | "model" | "reasoning_effort" | "specialization" | "system_prompt" | "project_id" | "status"
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

async function syncEmployeeRuntime(employeeId: string) {
  const runtime = await getEmployeeRuntimeStatus(employeeId);
  useEmployeeStore.setState((state) => ({
    employees: state.employees.map((employee) => (
      employee.id === employeeId
        ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
        : employee
    )),
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

  fetchEmployees: async () => {
    set({ loading: true });
    try {
      const employees = await select<Employee>("SELECT * FROM employees ORDER BY created_at");
      const runtimeResults = await Promise.allSettled(
        employees.map(async (employee) => [employee.id, await getEmployeeRuntimeStatus(employee.id)] as const),
      );
      const runtimeMap = new Map(
        runtimeResults
          .filter((result): result is PromiseFulfilledResult<readonly [string, EmployeeRuntimeStatus]> => result.status === "fulfilled")
          .map((result) => result.value),
      );

      set((state) => ({
        employees: employees.map((employee) => {
          const runtime = runtimeMap.get(employee.id);
          return runtime
            ? { ...employee, status: deriveEmployeeRuntimeStatus(employee, runtime) }
            : employee;
        }),
        employeeRuntime: employees.reduce<Record<string, EmployeeRuntimeStatus>>((acc, employee) => {
          const runtime = runtimeMap.get(employee.id);
          if (runtime) {
            acc[employee.id] = runtime;
          } else if (state.employeeRuntime[employee.id]) {
            acc[employee.id] = state.employeeRuntime[employee.id];
          }
          return acc;
        }, { ...state.employeeRuntime }),
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
    set((state) => {
      const { [id]: _runtime, ...employeeRuntime } = state.employeeRuntime;
      return { employeeRuntime };
    });
    await get().fetchEmployees();
  },

  updateEmployeeStatus: async (id, status) => {
    const employee = await updateEmployeeStatusCommand(id, status);
    set((state) => ({
      employees: state.employees.map((current) => (
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
          : current
      )),
    }));
  },

  addCodexOutput: (_employeeId, line, taskId, sessionKind = "execution", sessionRecordId, sessionEventId) => {
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
        onCodexSession((session: CodexSession) => {
          set((state) => ({
            employees: state.employees.map((employee) => (
              employee.id === session.employee_id
                ? { ...employee, status: "busy" }
                : employee
            )),
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
