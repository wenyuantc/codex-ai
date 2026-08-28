import { describe, expect, it } from "vitest";

import {
  aiProviderBadgeVariant,
  formatAiProviderLabel,
  formatSessionKind,
  formatSessionStatus,
  formatSessionTokenUsage,
  getStoredSessionsViewMode,
  matchesSessionIdentifier,
  normalizeSearchText,
  sessionDisplayKind,
  sessionKindBadgeClassName,
  sessionStatusBadgeVariant,
} from "@/lib/sessions";
import type { CodexSessionListItem } from "@/lib/types";

function sessionWithIds(ids: {
  session_id?: string | null;
  session_record_id?: string | null;
  cli_session_id?: string | null;
}): CodexSessionListItem {
  return {
    session_id: ids.session_id ?? null,
    session_record_id: ids.session_record_id ?? null,
    cli_session_id: ids.cli_session_id ?? null,
  } as CodexSessionListItem;
}

describe("normalizeSearchText", () => {
  it("lowercases and trims input", () => {
    expect(normalizeSearchText("  ABC-123  ")).toBe("abc-123");
  });

  it("treats null and undefined as empty", () => {
    expect(normalizeSearchText(null)).toBe("");
    expect(normalizeSearchText(undefined)).toBe("");
  });
});

describe("matchesSessionIdentifier", () => {
  const session = sessionWithIds({
    session_id: "cli-ABC",
    session_record_id: "rec-123",
    cli_session_id: "uuid-XYZ",
  });

  it("matches any of the session identifiers case-insensitively", () => {
    expect(matchesSessionIdentifier(session, "CLI-abc")).toBe(true);
    expect(matchesSessionIdentifier(session, "REC-123")).toBe(true);
    expect(matchesSessionIdentifier(session, "uuid-xyz")).toBe(true);
  });

  it("requires an exact match, not a substring", () => {
    expect(matchesSessionIdentifier(session, "rec-12")).toBe(false);
  });

  it("returns false for empty or null queries", () => {
    expect(matchesSessionIdentifier(session, "")).toBe(false);
    expect(matchesSessionIdentifier(session, null)).toBe(false);
  });
});

describe("getStoredSessionsViewMode", () => {
  it("falls back to card view when no window is available", () => {
    // Node test env has no localStorage; first-visit default must be card.
    expect(getStoredSessionsViewMode()).toBe("card");
  });
});

describe("sessionStatusBadgeVariant", () => {
  it("marks running as default and failed as destructive", () => {
    expect(sessionStatusBadgeVariant("running")).toBe("default");
    expect(sessionStatusBadgeVariant("failed")).toBe("destructive");
    expect(sessionStatusBadgeVariant("stopping")).toBe("secondary");
    expect(sessionStatusBadgeVariant("exited")).toBe("outline");
  });

  it("falls back to outline for unknown statuses", () => {
    expect(sessionStatusBadgeVariant("brand_new_status")).toBe("outline");
  });
});

describe("aiProviderBadgeVariant", () => {
  it("distinguishes codex and claude from other providers", () => {
    expect(aiProviderBadgeVariant("codex")).toBe("default");
    expect(aiProviderBadgeVariant("claude")).toBe("secondary");
    expect(aiProviderBadgeVariant("opencode")).toBe("outline");
    expect(aiProviderBadgeVariant("grok")).toBe("outline");
  });
});

describe("formatSessionTokenUsage", () => {
  it("shows unknown when total tokens are missing instead of pretending 0", () => {
    expect(formatSessionTokenUsage({ total_tokens: null })).toBe("未知");
    expect(formatSessionTokenUsage({})).toBe("未知");
  });

  it("formats known totals with cache usage and cache rate", () => {
    expect(formatSessionTokenUsage({ total_tokens: 950 })).toBe(
      "950 · 入 未知 / 出 未知 · 缓存 未知 · 率 未知",
    );
    expect(
      formatSessionTokenUsage({ total_tokens: 12340, input_tokens: 8000, output_tokens: 4340 }),
    ).toBe("12K · 入 8.0K / 出 4.3K · 缓存 未知 · 率 未知");
    expect(
      formatSessionTokenUsage({
        total_tokens: 110,
        input_tokens: 100,
        output_tokens: 10,
        cached_tokens: 25,
      }),
    ).toBe("110 · 入 100 / 出 10 · 缓存 25 · 率 25.0%");
    expect(
      formatSessionTokenUsage({
        total_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        cached_tokens: 0,
      }),
    ).toBe("0 · 入 0 / 出 0 · 缓存 0 · 率 —");
  });
});

describe("formatAiProviderLabel", () => {
  it("uses the provider option label", () => {
    expect(formatAiProviderLabel("codex")).toBe("Codex (OpenAI)");
    expect(formatAiProviderLabel("native")).toBe("内置 Agent");
  });
});

describe("formatSessionStatus", () => {
  it("maps known statuses to Chinese labels", () => {
    expect(formatSessionStatus("running")).toBe("运行中");
    expect(formatSessionStatus("failed")).toBe("失败");
  });

  it("falls back to the raw status for unknown values", () => {
    // Rendered directly in the UI, so an unmapped status shows as-is rather
    // than crashing — same fallback contract as getActivityActionLabel.
    expect(formatSessionStatus("brand_new_status")).toBe("brand_new_status");
  });
});

describe("sessionDisplayKind", () => {
  it("treats review as review even when origin is pipeline", () => {
    expect(sessionDisplayKind({ session_kind: "review", session_origin: "pipeline" })).toBe(
      "review",
    );
  });

  it("treats pipeline origin as pipeline for execution sessions", () => {
    expect(sessionDisplayKind({ session_kind: "execution", session_origin: "pipeline" })).toBe(
      "pipeline",
    );
  });

  it("treats coordinator kind as coordinator even when origin is pipeline", () => {
    expect(sessionDisplayKind({ session_kind: "coordinator", session_origin: "pipeline" })).toBe(
      "coordinator",
    );
    expect(sessionDisplayKind({ session_kind: "coordinator", session_origin: "direct" })).toBe(
      "coordinator",
    );
  });

  it("defaults missing or unknown origin to execution", () => {
    expect(sessionDisplayKind({ session_kind: "execution" })).toBe("execution");
    expect(sessionDisplayKind({ session_kind: "execution", session_origin: null })).toBe(
      "execution",
    );
    expect(sessionDisplayKind({ session_kind: "execution", session_origin: "other" })).toBe(
      "execution",
    );
    expect(sessionDisplayKind({})).toBe("execution");
  });
});

describe("formatSessionKind", () => {
  it("maps display kinds to i18n labels with review winning over pipeline", () => {
    expect(formatSessionKind({ session_kind: "review", session_origin: "pipeline" })).toBe("审核");
    expect(formatSessionKind({ session_kind: "execution", session_origin: "pipeline" })).toBe(
      "编排",
    );
    expect(formatSessionKind({ session_kind: "execution" })).toBe("执行");
    expect(formatSessionKind({ session_kind: "coordinator" })).toBe("协调");
  });
});

describe("sessionKindBadgeClassName", () => {
  it("uses distinct colors for review, pipeline, coordinator, and execution", () => {
    expect(sessionKindBadgeClassName("review")).toContain("text-blue-700");
    expect(sessionKindBadgeClassName("pipeline")).toContain("text-violet-700");
    expect(sessionKindBadgeClassName("coordinator")).toContain("text-teal-700");
    expect(sessionKindBadgeClassName("execution")).toContain("text-emerald-700");
  });
});
