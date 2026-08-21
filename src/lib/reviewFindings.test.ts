import { describe, expect, it } from "vitest";

import { matchReviewFindingToChange, normalizeReviewPath } from "@/lib/reviewFindings";
import type {
  CodexSessionFileChange,
  CodexSessionRecord,
  ReviewFinding,
  TaskExecutionChangeHistoryItem,
} from "@/lib/types";

function session(partial: Pick<CodexSessionRecord, "id" | "started_at">): CodexSessionRecord {
  return {
    id: partial.id,
    employee_id: null,
    task_id: "task-1",
    project_id: "proj-1",
    task_git_context_id: null,
    cli_session_id: null,
    working_dir: "/repo",
    session_kind: "execution",
    status: "exited",
    started_at: partial.started_at,
    ended_at: partial.started_at,
    exit_code: 0,
    resume_session_id: null,
    ai_provider: "codex",
    thinking_budget_tokens: null,
    input_tokens: null,
    output_tokens: null,
    total_tokens: null,
    reasoning_tokens: null,
    cached_tokens: null,
    execution_target: "local",
    ssh_config_id: null,
    target_host_label: null,
    artifact_capture_mode: "local_full",
    created_at: partial.started_at,
  };
}

function change(
  partial: Pick<CodexSessionFileChange, "id" | "session_id" | "path"> &
    Partial<Pick<CodexSessionFileChange, "previous_path">>,
): CodexSessionFileChange {
  return {
    id: partial.id,
    session_id: partial.session_id,
    path: partial.path,
    change_type: "modified",
    capture_mode: "sdk_event",
    previous_path: partial.previous_path ?? null,
    created_at: "2026-08-12 10:00:00",
  };
}

function historyItem(
  startedAt: string,
  sessionId: string,
  changes: CodexSessionFileChange[],
): TaskExecutionChangeHistoryItem {
  return {
    session: session({ id: sessionId, started_at: startedAt }),
    capture_mode: "sdk_event",
    changes,
  };
}

function finding(file: string): ReviewFinding {
  return {
    file,
    line: 12,
    severity: "warning",
    message: "check this",
  };
}

describe("normalizeReviewPath", () => {
  it("normalizes slashes, leading ./ and whitespace", () => {
    expect(normalizeReviewPath("  .\\src\\lib\\foo.ts  ")).toBe("src/lib/foo.ts");
    expect(normalizeReviewPath("./src/lib/foo.ts")).toBe("src/lib/foo.ts");
  });
});

describe("matchReviewFindingToChange", () => {
  const older = historyItem("2026-08-11 09:00:00", "sess-old", [
    change({ id: "old-src", session_id: "sess-old", path: "src/lib/foo.ts" }),
    change({
      id: "old-renamed",
      session_id: "sess-old",
      path: "src/lib/bar.ts",
      previous_path: "src/lib/legacy.ts",
    }),
  ]);
  const newer = historyItem("2026-08-12 11:00:00", "sess-new", [
    change({ id: "new-abs", session_id: "sess-new", path: "/repo/src/lib/foo.ts" }),
    change({ id: "new-other", session_id: "sess-new", path: "src/other.ts" }),
  ]);

  it("matches exact path in the newest session first", () => {
    const newerExact = historyItem("2026-08-12 11:00:00", "sess-new-exact", [
      change({ id: "new-exact", session_id: "sess-new-exact", path: "src/lib/foo.ts" }),
    ]);
    expect(matchReviewFindingToChange(finding("src/lib/foo.ts"), [older, newerExact])?.id).toBe(
      "new-exact",
    );
  });

  it("matches previous_path when current path differs", () => {
    expect(matchReviewFindingToChange(finding("src/lib/legacy.ts"), [older])?.id).toBe(
      "old-renamed",
    );
  });

  it("matches suffix when one path is absolute and the other is relative", () => {
    expect(matchReviewFindingToChange(finding("src/lib/foo.ts"), [newer])?.id).toBe("new-abs");
    expect(matchReviewFindingToChange(finding("/repo/src/other.ts"), [newer])?.id).toBe(
      "new-other",
    );
  });

  it("prefers the newest session when multiple sessions match", () => {
    expect(matchReviewFindingToChange(finding("src/lib/foo.ts"), [older, newer])?.id).toBe(
      "new-abs",
    );
  });

  it("returns null when nothing matches", () => {
    expect(matchReviewFindingToChange(finding("missing.rs"), [older, newer])).toBeNull();
    expect(matchReviewFindingToChange(finding("   "), [older])).toBeNull();
  });
});
