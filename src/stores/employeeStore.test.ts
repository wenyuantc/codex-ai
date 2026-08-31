import { describe, expect, it } from "vitest";

import type { NativeTextDelta } from "@/lib/native";
import type { Employee, EmployeeRuntimeStatus } from "@/lib/types";
import { applyEmployeeDeleted, applyStreamingDelta } from "@/stores/employeeStore";

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

function delta(overrides: Partial<NativeTextDelta> = {}): NativeTextDelta {
  return {
    employee_id: "emp-1",
    task_id: "task-1",
    session_kind: "execution",
    session_record_id: "sess-1",
    segment: "text",
    delta: "",
    clear: false,
    ...overrides,
  };
}

describe("applyStreamingDelta", () => {
  it("appends each segment under both the session and task terminal keys", () => {
    let state = applyStreamingDelta({}, delta({ segment: "reasoning", delta: "思" }));
    state = applyStreamingDelta(state, delta({ segment: "reasoning", delta: "考" }));
    state = applyStreamingDelta(state, delta({ delta: "答案" }));

    expect(state).toEqual({
      "sess-1": { reasoning: "思考", text: "答案" },
      "task-1::execution": { reasoning: "思考", text: "答案" },
    });
  });

  it("only tracks the session key when the session has no task", () => {
    const state = applyStreamingDelta({}, delta({ task_id: null, delta: "hi" }));

    expect(state).toEqual({ "sess-1": { reasoning: "", text: "hi" } });
  });

  it("drops both keys when the committed line arrives", () => {
    const streamed = applyStreamingDelta({}, delta({ delta: "partial" }));

    expect(applyStreamingDelta(streamed, delta({ clear: true }))).toEqual({});
  });

  it("keeps the same snapshot when there is nothing to apply", () => {
    const state = { "sess-2": { reasoning: "", text: "kept" } };

    expect(applyStreamingDelta(state, delta({ delta: "" }))).toBe(state);
    expect(applyStreamingDelta(state, delta({ clear: true }))).toBe(state);
  });
});
