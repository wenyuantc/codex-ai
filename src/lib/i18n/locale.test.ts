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
    expect(getActivityActionLabel("task_file_refs_added")).toBe("添加项目文件引用");
    expect(getActivityActionLabel("task_file_ref_removed")).toBe("移除项目文件引用");
    expect(getActivityActionLabel("native_plan_mode_entered")).toBe("内置 Agent 计划运行");
    expect(getActivityActionLabel("native_plan_mode_executed")).toBe("内置 Agent 开始执行计划");
    expect(getActivityActionLabel("native_plan_question_asked")).toBe("内置 Agent 计划提问");
    expect(getActivityActionLabel("native_plan_content_saved")).toBe("保存计划运行内容");
    expect(getActivityActionLabel("native_session_resumable")).toBe("内置 Agent 会话可续聊");
    expect(getActivityActionLabel("notification_sound_settings_updated")).toBe("更新通知声音设置");
    expect(getActivityActionLabel("native_token_diagnostics")).toBe("内置 Agent Token 诊断");
    expect(getActivityActionLabel("api_call_logs_viewed")).toBe("查看 API 调用记录");
    expect(i18n.t("nav:apiLogs")).toBe("API 调用记录");
    expect(i18n.t("apiLogs:listTitle")).toBe("API 调用记录");
    expect(i18n.t("apiLogs:lessThanOneSecond")).toBe("<1s");
    expect(i18n.t("apiLogs:callKind.one_shot")).toBe("一次性调用");
    expect(i18n.t("tasks:card.planRun")).toBe("计划运行");
    expect(i18n.t("tasks:detail.overview.openCoordinatorPlan")).toBe("打开协调员计划");
    expect(i18n.t("sessions:kindCoordinator")).toBe("协调");
    expect(i18n.t("tasks:detail.chain.role.coordinator")).toBe("协调");
    expect(i18n.t("tasks:detail.overview.viewCoordinatorPlanProgress")).toBe("查看生成进度");
    expect(i18n.t("tasks:nativePlanRunConfirm.continueExisting")).toBe("按已有计划继续");
    expect(i18n.t("tasks:nativePlanQuestion.submit")).toBe("继续");
    expect(i18n.t("tasks:nativePlanQuestion.other")).toBe("其他");
    expect(i18n.t("tasks:createDialog.selectProjectFiles")).toBe("选择项目文件");
    expect(i18n.t("settings:tabs.subagents")).toBe("子智能体");
    expect(i18n.t("settings:tabs.notifications")).toBe("通知");
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
    expect(i18n.t("tasks:nativePermission.allowServer")).toBe("本会话允许该 MCP 服务器");
    expect(i18n.t("tasks:nativePermission.heuristicNotice")).toContain("启发式");
    expect(i18n.t("settings:nativeAgent.confirmHighRiskLabel")).toBe("高风险工具需确认");
    expect(i18n.t("settings:nativeAgent.maxConcurrentSubagentsLabel")).toBe("同轮子 Agent 上限");
    expect(i18n.t("settings:nativeAgent.subagentPolicyLabel")).toBe("子 Agent 策略");
    expect(i18n.t("settings:nativeAgent.subagentPolicyBalanced")).toBe("均衡");
    expect(i18n.t("settings:page.engineCapabilities.notes.claude")).toContain(
      "本地 CLI/SDK 可附带图片",
    );
    expect(i18n.t("settings:prompts.scenes.native_agent_global")).toBe("内置 Agent 全局提示词");
    expect(i18n.t("settings:channels.fields.thinkingLevels")).toBe("允许的思考等级");
    expect(i18n.t("settings:channels.fields.thinkingLevelsEmpty")).toContain("至少勾选");
    expect(i18n.t("settings:channels.thinkingLevels.none")).toBe("无");
    expect(i18n.t("settings:channels.thinkingLevels.no_think")).toBe("不思考");

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
    expect(getActivityActionLabel("task_file_refs_added")).toBe("Added project file references");
    expect(getActivityActionLabel("task_file_ref_removed")).toBe("Removed project file reference");
    expect(getActivityActionLabel("native_plan_mode_entered")).toBe(
      "Started built-in Agent plan run",
    );
    expect(getActivityActionLabel("native_plan_mode_executed")).toBe(
      "Built-in Agent started executing plan",
    );
    expect(getActivityActionLabel("native_plan_question_asked")).toBe(
      "Built-in Agent asked a plan question",
    );
    expect(getActivityActionLabel("native_plan_content_saved")).toBe("Saved plan-run content");
    expect(getActivityActionLabel("native_session_resumable")).toBe(
      "Built-in Agent session can be resumed",
    );
    expect(getActivityActionLabel("notification_sound_settings_updated")).toBe(
      "Updated notification sound settings",
    );
    expect(getActivityActionLabel("native_token_diagnostics")).toBe(
      "Built-in Agent token diagnostics",
    );
    expect(getActivityActionLabel("api_call_logs_viewed")).toBe("Viewed API call logs");
    expect(i18n.t("nav:apiLogs")).toBe("API Call Logs");
    expect(i18n.t("apiLogs:listTitle")).toBe("API Call Logs");
    expect(i18n.t("apiLogs:lessThanOneSecond")).toBe("<1s");
    expect(i18n.t("apiLogs:callKind.one_shot")).toBe("One-shot");
    expect(i18n.t("tasks:card.planRun")).toBe("Plan and run");
    expect(i18n.t("tasks:detail.overview.openCoordinatorPlan")).toBe("Open coordinator plan");
    expect(i18n.t("sessions:kindCoordinator")).toBe("Coordinator");
    expect(i18n.t("tasks:detail.chain.role.coordinator")).toBe("Coordinator");
    expect(i18n.t("tasks:detail.overview.viewCoordinatorPlanProgress")).toBe(
      "View generation progress",
    );
    expect(i18n.t("tasks:nativePlanRunConfirm.continueExisting")).toBe("Continue with saved plan");
    expect(i18n.t("tasks:nativePlanQuestion.submit")).toBe("Continue");
    expect(i18n.t("tasks:nativePlanQuestion.other")).toBe("Other");
    expect(i18n.t("tasks:createDialog.selectProjectFiles")).toBe("Select project files");
    expect(i18n.t("settings:tabs.subagents")).toBe("Sub-agents");
    expect(i18n.t("settings:tabs.notifications")).toBe("Notifications");
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
    expect(i18n.t("settings:channels.fields.thinkingLevels")).toBe("Allowed thinking levels");
    expect(i18n.t("settings:channels.fields.thinkingLevelsEmpty")).toContain("at least one");
    expect(i18n.t("settings:channels.thinkingLevels.none")).toBe("None");
    expect(i18n.t("settings:channels.thinkingLevels.no_think")).toBe("No think");

    await changeAppLocale("zh-CN");
  });
});
