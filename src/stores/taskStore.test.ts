import { describe, it, expect } from "vitest";

import { filterTasksByVisibleProjects, resolveTaskListRefreshProjectId } from "@/stores/taskStore";

const rows = [
  { id: "t1", project_id: "p1" },
  { id: "t2", project_id: "p2" },
  { id: "t3", project_id: "p1" },
];

describe("filterTasksByVisibleProjects", () => {
  it("keeps only rows whose project is visible", () => {
    expect(filterTasksByVisibleProjects(rows, new Set(["p1"]))).toEqual([
      { id: "t1", project_id: "p1" },
      { id: "t3", project_id: "p1" },
    ]);
  });

  it("returns nothing when no project is visible", () => {
    // Guards the local/SSH switch: after switching environment the project store
    // is scoped, so tasks from the other environment must disappear.
    expect(filterTasksByVisibleProjects(rows, new Set())).toEqual([]);
  });

  it("keeps every row when all projects are visible", () => {
    expect(filterTasksByVisibleProjects(rows, new Set(["p1", "p2"]))).toEqual(rows);
  });

  it("does not mutate the input array", () => {
    const input = [...rows];
    filterTasksByVisibleProjects(input, new Set(["p2"]));
    expect(input).toEqual(rows);
  });
});

describe("resolveTaskListRefreshProjectId", () => {
  it("treats explicit undefined refresh as all-projects, not the last fetch scope", () => {
    expect(resolveTaskListRefreshProjectId({ refreshProjectId: undefined }, "gb")).toBeUndefined();
  });

  it("uses an explicit project id even when a different scope is active", () => {
    expect(resolveTaskListRefreshProjectId({ refreshProjectId: "gb" }, "other")).toBe("gb");
  });

  it("falls back to the active board scope when options omit refreshProjectId", () => {
    expect(resolveTaskListRefreshProjectId(undefined, "gb")).toBe("gb");
    expect(resolveTaskListRefreshProjectId({}, "gb")).toBe("gb");
  });

  it("keeps all-projects when both the option and the active scope are empty", () => {
    expect(resolveTaskListRefreshProjectId(undefined, undefined)).toBeUndefined();
    expect(
      resolveTaskListRefreshProjectId({ refreshProjectId: undefined }, undefined),
    ).toBeUndefined();
  });
});
