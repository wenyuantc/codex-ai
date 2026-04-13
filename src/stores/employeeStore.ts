import { create } from "zustand";
import { select, execute } from "@/lib/database";
import type { Employee, ReasoningEffort, CodexModelId } from "@/lib/types";
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
  fetchEmployees: () => Promise<void>;
  createEmployee: (data: { name: string; role: string; model?: CodexModelId; reasoning_effort?: ReasoningEffort; specialization?: string; system_prompt?: string; project_id?: string }) => Promise<void>;
  updateEmployee: (id: string, updates: Partial<Pick<Employee, "name" | "role" | "model" | "reasoning_effort" | "specialization" | "system_prompt" | "project_id" | "status">>) => Promise<void>;
  deleteEmployee: (id: string) => Promise<void>;
  updateEmployeeStatus: (id: string, status: string) => Promise<void>;
  addCodexOutput: (employeeId: string, line: string) => void;
  setCodexRunning: (employeeId: string, running: boolean, activeTaskId?: string | null) => void;
  clearCodexOutput: (employeeId: string) => void;
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

export const useEmployeeStore = create<EmployeeStore>((set, get) => ({
  employees: [],
  loading: false,
  codexProcesses: {},

  fetchEmployees: async () => {
    set({ loading: true });
    try {
      const employees = await select<Employee>("SELECT * FROM employees ORDER BY created_at");
      set({ employees, loading: false });
    } catch (e) {
      console.error("Failed to fetch employees:", e);
      set({ loading: false });
    }
  },

  createEmployee: async (data) => {
    const id = crypto.randomUUID();
    await execute(
      "INSERT INTO employees (id, name, role, model, reasoning_effort, specialization, system_prompt, project_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
      [id, data.name, data.role, data.model ?? "gpt-5.4", data.reasoning_effort ?? "high", data.specialization ?? null, data.system_prompt ?? null, data.project_id ?? null]
    );
    await get().fetchEmployees();
  },

  updateEmployee: async (id, updates) => {
    const fields: string[] = [];
    const values: unknown[] = [];
    let idx = 1;
    for (const [key, value] of Object.entries(updates)) {
      fields.push(`${key} = $${idx}`);
      values.push(value);
      idx++;
    }
    values.push(id);
    await execute(`UPDATE employees SET ${fields.join(", ")} WHERE id = $${idx}`, values);
    await get().fetchEmployees();
  },

  deleteEmployee: async (id) => {
    await execute("DELETE FROM employees WHERE id = $1", [id]);
    set((state) => {
      const { [id]: _, ...rest } = state.codexProcesses;
      return { codexProcesses: rest };
    });
    await get().fetchEmployees();
  },

  updateEmployeeStatus: async (id, status) => {
    await execute("UPDATE employees SET status = $1 WHERE id = $2", [status, id]);
    set((state) => ({
      employees: state.employees.map((e) => (e.id === id ? { ...e, status } : e)),
    }));
  },

  addCodexOutput: (employeeId, line) => {
    set((state) => ({
      codexProcesses: {
        ...state.codexProcesses,
        [employeeId]: {
          ...state.codexProcesses[employeeId],
          output: [...(state.codexProcesses[employeeId]?.output ?? []).slice(-199), line],
          running: state.codexProcesses[employeeId]?.running ?? false,
        },
      },
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

  initCodexListeners: () => {
    codexListenerRefCount += 1;

    if (!codexListenersInitPromise && !codexListenersCleanup) {
      codexListenersInitPromise = Promise.all([
        onCodexOutput((output: CodexOutput) => {
          get().addCodexOutput(output.employee_id, output.line);
        }),
        onCodexExit((exit) => {
          get().setCodexRunning(exit.employee_id, false, null);
          get().addCodexOutput(exit.employee_id, `[EXIT] Code: ${exit.code ?? "unknown"}`);
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
