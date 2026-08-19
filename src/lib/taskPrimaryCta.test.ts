import { describe, expect, it } from "vitest";

import { resolveTaskPrimaryCta } from "@/lib/taskPrimaryCta";

const base = {
  status: "todo",
  executionActive: false,
  reviewActive: false,
  canStopProcess: false,
  hasAssignee: true,
  hasReviewer: false,
  canCommit: false,
  canGenerateAcceptance: false,
};

describe("resolveTaskPrimaryCta", () => {
  it("shows stop when this task process can be stopped", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      status: "in_progress",
      canStopProcess: true,
    });
    expect(cta.kind).toBe("stop");
    expect(cta.label).toBe("停止");
    expect(cta.disabled).toBe(false);
  });

  it("locks with 运行中 when this task execution is active but not stoppable", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      status: "in_progress",
      executionActive: true,
    });
    expect(cta.kind).toBe("running_locked");
    expect(cta.label).toBe("运行中");
    expect(cta.disabled).toBe(true);
  });

  it("offers 并行运行 when assignee is busy on another task", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      assigneeBusyOnOtherTask: true,
    });
    expect(cta.kind).toBe("run");
    expect(cta.label).toBe("并行运行");
    expect(cta.disabled).toBe(false);
    expect(cta.reason).toContain("同员工另有任务运行中");
  });

  it("disables run when dependencies are incomplete", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      hasIncompleteDependencies: true,
      incompleteDependencySummary: "任务 A",
    });
    expect(cta.kind).toBe("run");
    expect(cta.disabled).toBe(true);
    expect(cta.reason).toContain("任务 A");
  });

  it("disables review when SSH artifact evidence is limited", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      status: "review",
      hasReviewer: true,
      sshReviewEvidenceLimited: true,
    });
    expect(cta.kind).toBe("review");
    expect(cta.disabled).toBe(true);
    expect(cta.reason).toContain("SSH");
  });

  it("disables run without assignee", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      hasAssignee: false,
    });
    expect(cta.disabled).toBe(true);
    expect(cta.reason).toBe("请先指派员工");
  });

  it("locks with 排队中 when the task is queued", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      queued: true,
    });
    expect(cta.kind).toBe("queued");
    expect(cta.label).toBe("排队中");
    expect(cta.disabled).toBe(true);
  });

  it("prefers stop over queued", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      canStopProcess: true,
      queued: true,
    });
    expect(cta.kind).toBe("stop");
  });

  it("prefers starting over queued", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      backgroundStarting: true,
      queued: true,
    });
    expect(cta.kind).toBe("starting");
  });

  it("prefers running_locked over queued", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      executionActive: true,
      queued: true,
    });
    expect(cta.kind).toBe("running_locked");
  });

  it("prefers queued over ordinary run", () => {
    const cta = resolveTaskPrimaryCta({
      ...base,
      queued: true,
      assigneeBusyOnOtherTask: true,
    });
    expect(cta.kind).toBe("queued");
    expect(cta.disabled).toBe(true);
  });
});
