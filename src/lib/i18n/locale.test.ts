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
    expect(getStatusLabel("todo")).toBe("待办");
    expect(getStatusLabel("offline")).toBe("空闲");
    expect(getStatusLabel("busy")).toBe("运行中");
    expect(getStatusLabel("online")).toBe("运行中");
    expect(getStatusLabel("error")).toBe("异常");
    expect(i18n.t("dashboard:stats.onlineEmployees")).toBe("运行中员工");
    expect(i18n.t("employees:noEnabledChannelHint")).toContain("AI 渠道");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("进程内");
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
    expect(getStatusLabel("todo")).toBe("To do");
    expect(getStatusLabel("offline")).toBe("Idle");
    expect(getStatusLabel("busy")).toBe("Running");
    expect(getStatusLabel("online")).toBe("Running");
    expect(i18n.t("dashboard:stats.onlineEmployees")).toBe("Running employees");
    expect(i18n.t("nav:dashboard")).toBe("Dashboard");
    expect(i18n.t("employees:noEnabledChannelHint")).toContain("AI channels");
    expect(i18n.t("settings:page.engineCapabilities.notes.native")).toContain("in-process");
    expect(i18n.t("settings:prompts.scenes.native_agent_global")).toBe(
      "Built-in Agent global prompt",
    );

    await changeAppLocale("zh-CN");
  });
});
