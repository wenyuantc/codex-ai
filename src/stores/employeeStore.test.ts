import { describe, expect, it } from "vitest";

import type { Employee, EmployeeRuntimeStatus } from "@/lib/types";
import { applyEmployeeDeleted } from "@/stores/employeeStore";

function employee(id: string): Employee {
  return {
    id,
    name: id,
    role: "developer",
    model: "gpt-5.4",
    reasoning_effort: "high",
    status: "offline",
    specialization: null,
    system_prompt: null,
    project_id: null,
    ai_provider: "codex",
    ai_channel_id: null,
    created_at: "2026-08-24 00:00:00",
    updated_at: "2026-08-24 00:00:00",
  };
}

function runtime(running = false): EmployeeRuntimeStatus {
  return {
    running,
    sessions: [],
    latest_session: null,
  };
}

describe("applyEmployeeDeleted", () => {
  it("removes the employee and their runtime in the same snapshot", () => {
    const keep = employee("emp-keep");
    const removed = employee("emp-gone");
    const next = applyEmployeeDeleted(
      {
        employees: [keep, removed],
        employeeRuntime: {
          "emp-keep": runtime(),
          "emp-gone": runtime(true),
        },
      },
      "emp-gone",
    );

    expect(next.employees).toEqual([keep]);
    expect(next.employeeRuntime).toEqual({ "emp-keep": runtime() });
    expect(next.employeeRuntime["emp-gone"]).toBeUndefined();
  });

  it("leaves other employees untouched when the id is already absent", () => {
    const keep = employee("emp-keep");
    const state = {
      employees: [keep],
      employeeRuntime: { "emp-keep": runtime() },
    };

    expect(applyEmployeeDeleted(state, "missing")).toEqual(state);
  });
});
