import { describe, it, expect } from "vitest";

import {
  formatDuration,
  getActivityActionLabel,
  isEmployeeRunningStatus,
  isTaskOverdue,
  matchesEmployeeRuntimeFilter,
  shouldShowTaskTimer,
} from "@/lib/utils";

describe("getActivityActionLabel", () => {
  it("maps known backend action keys to Chinese labels", () => {
    // Sampled across eras/domains: task lifecycle, automation, SSH, remote engine runtime.
    expect(getActivityActionLabel("task_created")).toBe("创建任务");
    expect(getActivityActionLabel("task_template_created")).toBe("创建任务模板");
    expect(getActivityActionLabel("task_template_applied")).toBe("套用任务模板");
    expect(getActivityActionLabel("task_template_deleted")).toBe("删除任务模板");
    expect(getActivityActionLabel("task_automation_completed")).toBe("自动质控闭环完成");
    expect(getActivityActionLabel("ssh_config_created")).toBe("新增SSH配置");
    expect(getActivityActionLabel("remote_opencode_validated")).toBe("远程校验 OpenCode");
    expect(getActivityActionLabel("session_events_purged")).toBe("清理会话事件");
    expect(getActivityActionLabel("app_update_installed")).toBe("应用已更新");
    expect(getActivityActionLabel("task_run_queued")).toBe("任务加入运行队列");
    expect(getActivityActionLabel("task_run_dequeued")).toBe("排队任务开始执行");
    expect(getActivityActionLabel("task_run_queue_cancelled")).toBe("取消运行排队");
    expect(getActivityActionLabel("task_run_dequeue_failed")).toBe("排队任务启动失败");
    expect(getActivityActionLabel("native_settings_updated")).toBe("更新内置 Agent 设置");
    expect(getActivityActionLabel("native_subagent_started")).toBe("启动子 Agent");
    expect(getActivityActionLabel("native_subagent_finished")).toBe("子 Agent 结束");
    expect(getActivityActionLabel("native_subagent_created")).toBe("创建子智能体");
    expect(getActivityActionLabel("native_subagent_updated")).toBe("更新子智能体");
    expect(getActivityActionLabel("native_subagent_deleted")).toBe("删除子智能体");
    expect(getActivityActionLabel("task_native_subagent_updated")).toBe("更新任务子智能体");
    expect(getActivityActionLabel("task_file_refs_added")).toBe("添加项目文件引用");
    expect(getActivityActionLabel("task_file_ref_removed")).toBe("移除项目文件引用");
    expect(getActivityActionLabel("native_plan_mode_entered")).toBe("内置 Agent 计划运行");
    expect(getActivityActionLabel("native_plan_mode_executed")).toBe("内置 Agent 开始执行计划");
    expect(getActivityActionLabel("native_plan_question_asked")).toBe("内置 Agent 计划提问");
    expect(getActivityActionLabel("native_plan_content_saved")).toBe("保存计划运行内容");
    expect(getActivityActionLabel("notification_sound_settings_updated")).toBe("更新通知声音设置");
  });

  it("falls back to the raw key for unmapped actions", () => {
    // The dashboard renders this return value directly, so an unmapped key is
    // visible as snake_case rather than crashing — regression guard for the
    // historical "missing Chinese label" bug.
    expect(getActivityActionLabel("brand_new_action_key")).toBe("brand_new_action_key");
  });

  it("does not treat an empty action as a mapped label", () => {
    expect(getActivityActionLabel("")).toBe("");
  });
});

describe("isTaskOverdue", () => {
  const today = "2026-08-06";

  it("marks a task past its due date as overdue", () => {
    expect(isTaskOverdue({ due_date: "2026-08-05", status: "in_progress" }, today)).toBe(true);
  });

  it("does not mark the due date itself as overdue", () => {
    expect(isTaskOverdue({ due_date: "2026-08-06", status: "in_progress" }, today)).toBe(false);
  });

  it("ignores completed and archived tasks regardless of due date", () => {
    expect(isTaskOverdue({ due_date: "2020-01-01", status: "completed" }, today)).toBe(false);
    expect(isTaskOverdue({ due_date: "2020-01-01", status: "archived" }, today)).toBe(false);
  });

  it("treats a missing due date as not overdue", () => {
    expect(isTaskOverdue({ due_date: null, status: "todo" }, today)).toBe(false);
  });

  it("accepts a datetime due_date and compares only the date part", () => {
    expect(isTaskOverdue({ due_date: "2026-08-05 23:59:59", status: "todo" }, today)).toBe(true);
  });
});

describe("employee runtime status", () => {
  it("treats busy and leftover online as running, offline as idle", () => {
    expect(isEmployeeRunningStatus("busy")).toBe(true);
    expect(isEmployeeRunningStatus("online")).toBe(true);
    expect(isEmployeeRunningStatus("offline")).toBe(false);
    expect(isEmployeeRunningStatus("error")).toBe(false);
  });

  it("matches the running filter without treating idle as running", () => {
    expect(matchesEmployeeRuntimeFilter("offline", "all")).toBe(true);
    expect(matchesEmployeeRuntimeFilter("online", "busy")).toBe(true);
    expect(matchesEmployeeRuntimeFilter("busy", "busy")).toBe(true);
    expect(matchesEmployeeRuntimeFilter("offline", "busy")).toBe(false);
    expect(matchesEmployeeRuntimeFilter("offline", "offline")).toBe(true);
    expect(matchesEmployeeRuntimeFilter("error", "error")).toBe(true);
  });
});

describe("shouldShowTaskTimer", () => {
  it("hides the idle timer until a run has started, spent time, or completed", () => {
    expect(
      shouldShowTaskTimer({
        time_started_at: null,
        time_spent_seconds: 0,
        completed_at: null,
      }),
    ).toBe(false);
    expect(shouldShowTaskTimer({ time_started_at: "2026-08-20T01:00:00Z" })).toBe(true);
    expect(shouldShowTaskTimer({ time_spent_seconds: 12 })).toBe(true);
    expect(shouldShowTaskTimer({ completed_at: "2026-08-20T02:00:00Z" })).toBe(true);
  });
});

describe("formatDuration", () => {
  it("formats sub-minute durations in seconds", () => {
    expect(formatDuration(0)).toBe("0秒");
    expect(formatDuration(45)).toBe("45秒");
  });

  it("drops the seconds part on an exact minute", () => {
    expect(formatDuration(60)).toBe("1分钟");
    expect(formatDuration(90)).toBe("1分钟30秒");
  });

  it("drops the seconds part entirely once hours are present", () => {
    expect(formatDuration(3600)).toBe("1小时");
    expect(formatDuration(3661)).toBe("1小时1分钟");
  });

  it("clamps nullish and negative input to zero", () => {
    expect(formatDuration(null)).toBe("0秒");
    expect(formatDuration(undefined)).toBe("0秒");
    expect(formatDuration(-30)).toBe("0秒");
  });
});
