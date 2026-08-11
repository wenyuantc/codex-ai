import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import {
  DEFAULT_LOCALE,
  getLocalePreference,
  persistLocalePreference,
  type AppLocale,
} from "./locale";

import commonZh from "@/locales/zh-CN/common.json";
import navZh from "@/locales/zh-CN/nav.json";
import activityZh from "@/locales/zh-CN/activity.json";
import settingsZh from "@/locales/zh-CN/settings.json";
import errorsZh from "@/locales/zh-CN/errors.json";
import sshZh from "@/locales/zh-CN/ssh.json";
import dashboardZh from "@/locales/zh-CN/dashboard.json";
import kanbanZh from "@/locales/zh-CN/kanban.json";
import sessionsZh from "@/locales/zh-CN/sessions.json";
import employeesZh from "@/locales/zh-CN/employees.json";
import projectsZh from "@/locales/zh-CN/projects.json";
import trashZh from "@/locales/zh-CN/trash.json";
import notificationsZh from "@/locales/zh-CN/notifications.json";
import searchZh from "@/locales/zh-CN/search.json";
import tasksZh from "@/locales/zh-CN/tasks.json";

import commonEn from "@/locales/en/common.json";
import navEn from "@/locales/en/nav.json";
import activityEn from "@/locales/en/activity.json";
import settingsEn from "@/locales/en/settings.json";
import errorsEn from "@/locales/en/errors.json";
import sshEn from "@/locales/en/ssh.json";
import dashboardEn from "@/locales/en/dashboard.json";
import kanbanEn from "@/locales/en/kanban.json";
import sessionsEn from "@/locales/en/sessions.json";
import employeesEn from "@/locales/en/employees.json";
import projectsEn from "@/locales/en/projects.json";
import trashEn from "@/locales/en/trash.json";
import notificationsEn from "@/locales/en/notifications.json";
import searchEn from "@/locales/en/search.json";
import tasksEn from "@/locales/en/tasks.json";

export const I18N_NAMESPACES = [
  "common",
  "nav",
  "activity",
  "settings",
  "errors",
  "ssh",
  "dashboard",
  "kanban",
  "sessions",
  "employees",
  "projects",
  "trash",
  "notifications",
  "search",
  "tasks",
] as const;

export type I18nNamespace = (typeof I18N_NAMESPACES)[number];

const resources = {
  "zh-CN": {
    common: commonZh,
    nav: navZh,
    activity: activityZh,
    settings: settingsZh,
    errors: errorsZh,
    ssh: sshZh,
    dashboard: dashboardZh,
    kanban: kanbanZh,
    sessions: sessionsZh,
    employees: employeesZh,
    projects: projectsZh,
    trash: trashZh,
    notifications: notificationsZh,
    search: searchZh,
    tasks: tasksZh,
  },
  en: {
    common: commonEn,
    nav: navEn,
    activity: activityEn,
    settings: settingsEn,
    errors: errorsEn,
    ssh: sshEn,
    dashboard: dashboardEn,
    kanban: kanbanEn,
    sessions: sessionsEn,
    employees: employeesEn,
    projects: projectsEn,
    trash: trashEn,
    notifications: notificationsEn,
    search: searchEn,
    tasks: tasksEn,
  },
};

const initialLocale = getLocalePreference();

if (!i18n.isInitialized) {
  void i18n.use(initReactI18next).init({
    resources,
    lng: initialLocale,
    fallbackLng: DEFAULT_LOCALE,
    defaultNS: "common",
    ns: [...I18N_NAMESPACES],
    interpolation: {
      escapeValue: false,
    },
    returnNull: false,
  });
}

async function syncWindowTitle(locale: AppLocale): Promise<void> {
  const title = i18n.t("common:appName", { lng: locale });
  if (typeof document !== "undefined") {
    document.title = title;
  }
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  } catch {
    // Browser / non-Tauri preview: document.title is enough.
  }
}

export async function changeAppLocale(locale: AppLocale): Promise<AppLocale> {
  persistLocalePreference(locale);
  await i18n.changeLanguage(locale);
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale === "en" ? "en" : "zh-CN";
  }
  await syncWindowTitle(locale);
  return locale;
}

export function getCurrentAppLocale(): AppLocale {
  const lng = i18n.resolvedLanguage ?? i18n.language;
  return lng === "en" ? "en" : "zh-CN";
}

if (typeof document !== "undefined") {
  document.documentElement.lang = initialLocale === "en" ? "en" : "zh-CN";
  void syncWindowTitle(initialLocale);
}

export { i18n };
export default i18n;
