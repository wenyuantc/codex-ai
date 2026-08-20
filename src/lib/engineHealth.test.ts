import { describe, expect, it } from "vitest";

import { isAnyEngineReady, readyEngineIds } from "@/lib/engineHealth";

describe("engine readiness helpers", () => {
  it("treats a single non-Codex engine as enough", () => {
    expect(isAnyEngineReady({ codex: false, claude: true, grok: false, opencode: false })).toBe(
      true,
    );
    expect(isAnyEngineReady({ codex: false, claude: false, grok: false, opencode: false })).toBe(
      false,
    );
  });

  it("lists only ready engines in a stable order", () => {
    expect(readyEngineIds({ codex: false, claude: true, grok: true, opencode: false })).toEqual([
      "claude",
      "grok",
    ]);
  });
});
