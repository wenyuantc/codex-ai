import { describe, expect, it } from "vitest";

import { nativeSubagentMatchesProject } from "./nativeSubagentScope";

describe("nativeSubagentMatchesProject", () => {
  it("treats missing or all scope as visible everywhere", () => {
    expect(nativeSubagentMatchesProject({ scope: "all" }, "p1")).toBe(true);
    expect(nativeSubagentMatchesProject({}, null)).toBe(true);
  });

  it("limits projects scope to listed ids", () => {
    const item = { scope: "projects", project_ids: ["alpha", "beta"] };
    expect(nativeSubagentMatchesProject(item, "alpha")).toBe(true);
    expect(nativeSubagentMatchesProject(item, "other")).toBe(false);
    expect(nativeSubagentMatchesProject(item, "")).toBe(false);
    expect(nativeSubagentMatchesProject(item, null)).toBe(false);
  });
});
