import { describe, expect, it } from "vitest";

import { changeAppLocale, getCurrentAppLocale, i18n } from "@/lib/i18n";
import { getLocalePreference, isAppLocale } from "@/lib/i18n/locale";
import { getActivityActionLabel, getStatusLabel } from "@/lib/utils";

describe("i18n locale preference", () => {
  it("defaults to zh-CN and recognizes supported locales", () => {
    expect(isAppLocale("zh-CN")).toBe(true);
    expect(isAppLocale("en")).toBe(true);
    expect(isAppLocale("ja")).toBe(false);
    expect(getLocalePreference()).toBe("zh-CN");
  });

  it("switches activity and status labels with language", async () => {
    await changeAppLocale("zh-CN");
    expect(getCurrentAppLocale()).toBe("zh-CN");
    expect(getActivityActionLabel("task_created")).toBe("创建任务");
    expect(getActivityActionLabel("app_update_installed")).toBe("应用已更新");
    expect(getActivityActionLabel("ai_channel_created")).toBe("新增 AI 渠道");
    expect(getActivityActionLabel("ai_channel_models_fetched")).toBe("拉取 AI 渠道模型");
    expect(getActivityActionLabel("native_settings_updated")).toBe("更新内置 Agent 设置");
    expect(getActivityActionLabel("native_subagent_started")).toBe("启动子 Agent");
    expect(getActivityActionLabel("native_subagent_finished")).toBe("子 Agent 结束");
    expect(getActivityActionLabel("native_subagent_created")).toBe("创建子智能体");
    expect(getActivityActionLabel("task_native_subagent_updated")).toBe("更新任务子智能体");
    expect(i18n.t("settings:tabs.subagents")).toBe("子智能体");
    expect(i18n.t("settings:subagents.dialogs.createTitle")).toBe("新建子智能体");
    expect(i18n.t("settings:subagents.dialogs.editTitle")).toBe("编辑子智能体");
    expect(i18n.t("settings:subagents.actions.edit")).toBe("编辑");
    expect(i18n.t("tasks:nativeSubagent.unspecified")).toBe("未指定");
    expect(i18n.t("settings:subagents.fields.modelInherit")).toContain("继承");
    expect(getStatusLabel("todo")).toBe("待办");
    expect(getStatusLabel("offline")).toBe("空闲");
    expect(getStatusLabel("busy")).toBe("运行中");
    expect(getStatusLabel("online")).toBe("运行中");
    expect(getStatusLabel("error")).toBe("异常");
    expect(i18n.t("dashboard:stats.onlineEmployees")).toBe("运行中员工");
    expect(i18n.t("employees:noEnabledChannelHint")).toContain("AI 渠道");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("进程内");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("MCP");
    expect(i18n.t("settings:mcp.description")).toContain("内置 Agent");
    expect(i18n.t("errors:capability.mcp")).toBe("MCP");
    expect(i18n.t("tasks:nativePermission.allowSession")).toBe("本会话全部允许");
    expect(i18n.t("settings:nativeAgent.confirmHighRiskLabel")).toBe("高风险工具需确认");
    expect(i18n.t("settings:nativeAgent.maxConcurrentSubagentsLabel")).toBe("同轮子 Agent 上限");
    expect(i18n.t("settings:nativeAgent.subagentPolicyLabel")).toBe("子 Agent 策略");
    expect(i18n.t("settings:nativeAgent.subagentPolicyBalanced")).toBe("均衡");
    expect(i18n.t("settings:page.engineCapabilities.notes.claude")).toContain(
      "本地 CLI/SDK 可附带图片",
    );
    expect(i18n.t("settings:prompts.scenes.native_agent_global")).toBe("内置 Agent 全局提示词");

    await changeAppLocale("en");
    expect(getCurrentAppLocale()).toBe("en");
    expect(getActivityActionLabel("task_created")).toBe("Created task");
    expect(getActivityActionLabel("app_update_installed")).toBe("App updated");
    expect(getActivityActionLabel("ai_channel_created")).toBe("Created AI channel");
    expect(getActivityActionLabel("ai_channel_models_fetched")).toBe("Fetched AI channel models");
    expect(getActivityActionLabel("native_settings_updated")).toBe(
      "Updated built-in Agent settings",
    );
    expect(getActivityActionLabel("native_subagent_started")).toBe("Started sub-agent");
    expect(getActivityActionLabel("native_subagent_finished")).toBe("Sub-agent finished");
    expect(getActivityActionLabel("native_subagent_created")).toBe("Created sub-agent");
    expect(getActivityActionLabel("task_native_subagent_updated")).toBe("Updated task sub-agent");
    expect(i18n.t("settings:tabs.subagents")).toBe("Sub-agents");
    expect(i18n.t("settings:subagents.dialogs.createTitle")).toBe("New sub-agent");
    expect(i18n.t("settings:subagents.dialogs.editTitle")).toBe("Edit sub-agent");
    expect(i18n.t("settings:subagents.actions.edit")).toBe("Edit");
    expect(i18n.t("tasks:nativeSubagent.unspecified")).toBe("Not set");
    expect(i18n.t("settings:subagents.fields.modelInherit")).toContain("Inherit");
    expect(getStatusLabel("todo")).toBe("To do");
    expect(getStatusLabel("offline")).toBe("Idle");
    expect(getStatusLabel("busy")).toBe("Running");
    expect(getStatusLabel("online")).toBe("Running");
    expect(i18n.t("dashboard:stats.onlineEmployees")).toBe("Running employees");
    expect(i18n.t("nav:dashboard")).toBe("Dashboard");
    expect(i18n.t("employees:noEnabledChannelHint")).toContain("AI channels");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("in-process");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("MCP");
    expect(i18n.t("settings:mcp.description")).toContain("built-in Agent");
    expect(i18n.t("errors:capability.mcp")).toBe("MCP");
    expect(i18n.t("tasks:nativePermission.allowOnce")).toBe("Allow once");
    expect(i18n.t("settings:nativeAgent.confirmHighRiskLabel")).toBe("Confirm high-risk tools");
    expect(i18n.t("settings:nativeAgent.maxConcurrentSubagentsLabel")).toBe("Sub-agents per turn");
    expect(i18n.t("settings:nativeAgent.subagentPolicyLabel")).toBe("Sub-agent policy");
    expect(i18n.t("settings:nativeAgent.subagentPolicyBalanced")).toBe("Balanced");
    expect(i18n.t("settings:page.engineCapabilities.notes.claude")).toContain(
      "Local CLI/SDK can attach images",
    );
    expect(i18n.t("settings:prompts.scenes.native_agent_global")).toBe(
      "Built-in Agent global prompt",
    );

    await changeAppLocale("zh-CN");
  });
});
