import { describe, it, expect } from "vitest";

import { filterTasksByVisibleProjects } from "@/stores/taskStore";

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
