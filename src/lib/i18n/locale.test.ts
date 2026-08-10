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
    expect(getStatusLabel("todo")).toBe("待办");

    await changeAppLocale("en");
    expect(getCurrentAppLocale()).toBe("en");
    expect(getActivityActionLabel("task_created")).toBe("Created task");
    expect(getStatusLabel("todo")).toBe("To do");
    expect(i18n.t("nav:dashboard")).toBe("Dashboard");

    await changeAppLocale("zh-CN");
  });
});
