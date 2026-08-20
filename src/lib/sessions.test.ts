import { describe, expect, it } from "vitest";

import {
  aiProviderBadgeVariant,
  formatAiProviderLabel,
  formatSessionStatus,
  formatSessionTokenUsage,
  getStoredSessionsViewMode,
  matchesSessionIdentifier,
  normalizeSearchText,
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

  it("formats a known total and optional input/output split", () => {
    expect(formatSessionTokenUsage({ total_tokens: 950 })).toBe("950");
    expect(
      formatSessionTokenUsage({ total_tokens: 12340, input_tokens: 8000, output_tokens: 4340 }),
    ).toBe("12K · 入 8.0K / 出 4.3K");
  });
});

describe("formatAiProviderLabel", () => {
  it("uses the provider option label", () => {
    expect(formatAiProviderLabel("codex")).toBe("Codex (OpenAI)");
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
