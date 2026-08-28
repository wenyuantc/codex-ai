import { describe, expect, it } from "vitest";

import { buildTaskSessionChain } from "@/components/tasks/detail/TaskSessionChainPanel";
import type { CodexSessionListItem } from "@/lib/types";

function session(
  partial: Pick<CodexSessionListItem, "session_record_id" | "session_kind"> &
    Partial<Pick<CodexSessionListItem, "session_origin" | "status" | "last_updated_at">>,
): CodexSessionListItem {
  return {
    session_record_id: partial.session_record_id,
    session_id: partial.session_record_id,
    cli_session_id: null,
    ai_provider: "native",
    session_kind: partial.session_kind,
    session_origin: partial.session_origin ?? "direct",
    status: partial.status ?? "exited",
    last_updated_at: partial.last_updated_at ?? "2026-04-28 09:00:00",
    display_name: partial.session_record_id,
    summary: null,
    content_preview: null,
    employee_id: "emp-1",
    employee_name: "协调员",
    task_id: "task-1",
    task_git_context_id: null,
    task_title: "任务",
    task_status: "in_progress",
    project_id: "proj-1",
    project_name: "项目",
    working_dir: "/repo",
    execution_target: "local",
    ssh_config_id: null,
    target_host_label: null,
    artifact_capture_mode: "local_full",
    input_tokens: null,
    output_tokens: null,
    total_tokens: null,
    reasoning_tokens: null,
    cached_tokens: null,
    resume_status: "missing_cli_session",
    resume_message: null,
    can_resume: false,
  };
}

describe("buildTaskSessionChain", () => {
  it("labels coordinator sessions as coordinator instead of execute or pipeline", () => {
    const chain = buildTaskSessionChain(
      [
        session({ session_record_id: "s-coord", session_kind: "coordinator" }),
        session({
          session_record_id: "s-pipe",
          session_kind: "execution",
          session_origin: "pipeline",
        }),
      ],
      [],
    );
    expect(chain.map((item) => item.role)).toEqual(["coordinator", "pipeline"]);
  });
});
