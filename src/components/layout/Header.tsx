import { useLocation, useNavigate } from "react-router-dom";
import { Laptop, Moon, ServerCog, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useTranslation } from "react-i18next";
import { GLOBAL_SHORTCUTS, shortcutDisplay, shortcutKeys } from "@/lib/shortcuts";
import { useProjectStore } from "@/stores/projectStore";
import {
  applyTheme,
  getThemePreference,
  isDarkThemeMode,
  THEME_CHANGE_EVENT,
  type ThemeMode,
} from "@/lib/theme";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { getEnvironmentModeLabel } from "@/lib/projects";
import { GlobalSearchDialog } from "@/components/search/GlobalSearchDialog";
import { NotificationCenter } from "@/components/layout/NotificationCenter";

const ALL_PROJECTS_VALUE = "__all_projects__";

const pageTitleKeys: Record<string, string> = {
  "/": "nav:dashboard",
  "/projects": "nav:projects",
  "/kanban": "nav:kanban",
  "/employees": "nav:employees",
  "/sessions": "nav:sessions",
  "/api-logs": "nav:apiLogs",
  "/settings": "nav:settingsFull",
  "/trash": "nav:trash",
};

export function Header() {
  const { t } = useTranslation(["nav", "common"]);
  const location = useLocation();
  const navigate = useNavigate();
  const titleKey = pageTitleKeys[location.pathname];
  const title = titleKey ? t(titleKey) : t("common:appName");
  const {
    projects,
    currentProject,
    environmentMode,
    sshConfigs,
    sshConfigsInitialized,
    selectedSshConfigId,
    setCurrentProject,
    setEnvironmentMode,
    setSelectedSshConfigId,
    fetchProjects,
    fetchSshConfigs,
  } = useProjectStore();
  const [themeMode, setThemeMode] = useState<ThemeMode>(getThemePreference);
  const dark = isDarkThemeMode(themeMode);

  useEffect(() => {
    void fetchProjects();
    void fetchSshConfigs();
  }, [fetchProjects, fetchSshConfigs]);

  useEffect(() => {
    const nextMode = getThemePreference();
    setThemeMode(nextMode);
    applyTheme(nextMode);

    const handleThemeChange = () => {
      setThemeMode(getThemePreference());
    };

    window.addEventListener(THEME_CHANGE_EVENT, handleThemeChange);

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      const preference = getThemePreference();
      if (preference === "system") {
        setThemeMode(preference);
        applyTheme(preference);
      }
    };

    mediaQuery.addEventListener("change", handleSystemThemeChange);
    return () => {
      window.removeEventListener(THEME_CHANGE_EVENT, handleThemeChange);
      mediaQuery.removeEventListener("change", handleSystemThemeChange);
    };
  }, []);

  useEffect(() => {
    if (
      environmentMode === "ssh" &&
      sshConfigsInitialized &&
      sshConfigs.length === 0 &&
      location.pathname !== "/settings"
    ) {
      navigate("/settings");
    }
  }, [environmentMode, location.pathname, navigate, sshConfigs.length, sshConfigsInitialized]);

  useHotkeys(shortcutKeys(GLOBAL_SHORTCUTS[1]), (e) => {
    e.preventDefault();
    toggleTheme();
  });

  const toggleTheme = () => {
    const nextMode: ThemeMode = dark ? "light" : "dark";
    setThemeMode(nextMode);
    applyTheme(nextMode);
  };

  const handleEnvironmentModeChange = async (nextMode: "local" | "ssh") => {
    const result = await setEnvironmentMode(nextMode);
    if (result.redirectToSettings) {
      navigate("/settings");
    }
  };

  return (
    <header className="flex items-center justify-between h-14 px-6 border-b border-border bg-background">
      <div className="flex items-center">
        <h1 className="text-lg font-semibold">{title}</h1>
      </div>

      <div className="flex items-center gap-4">
        <GlobalSearchDialog />
        <NotificationCenter />
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-muted-foreground">
            {getEnvironmentModeLabel(environmentMode)}
          </span>
          <div className="inline-flex rounded-md border border-border bg-muted/30 p-0.5">
            <Button
              type="button"
              variant={environmentMode === "local" ? "default" : "ghost"}
              size="sm"
              className="h-7 gap-1.5 px-2 text-xs"
              onClick={() => void handleEnvironmentModeChange("local")}
            >
              <Laptop className="h-3.5 w-3.5" />
              {t("common:localShort")}
            </Button>
            <Button
              type="button"
              variant={environmentMode === "ssh" ? "default" : "ghost"}
              size="sm"
              className="h-7 gap-1.5 px-2 text-xs"
              onClick={() => void handleEnvironmentModeChange("ssh")}
            >
              <ServerCog className="h-3.5 w-3.5" />
              {t("common:sshShort")}
            </Button>
          </div>
        </div>
        {environmentMode === "ssh" && sshConfigs.length > 0 && (
          <Select
            value={selectedSshConfigId || null}
            onValueChange={(value) => {
              if (!value) {
                return;
              }

              setSelectedSshConfigId(value);
            }}
          >
            <SelectTrigger className="w-[260px] bg-background">
              <SelectValue placeholder={t("common:selectSshHost")}>
                {(value) => {
                  if (!value) {
                    return t("common:selectSshHost");
                  }

                  const sshConfig = sshConfigs.find((config) => config.id === value);
                  return sshConfig
                    ? `${sshConfig.name} (${sshConfig.username}@${sshConfig.host})`
                    : t("common:selectSshHost");
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {sshConfigs.map((config) => (
                <SelectItem key={config.id} value={config.id}>
                  {config.name} ({config.username}@{config.host})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        {projects.length > 0 && (
          <Select
            value={currentProject?.id ?? ALL_PROJECTS_VALUE}
            onValueChange={(value) => {
              if (!value || value === ALL_PROJECTS_VALUE) {
                setCurrentProject(null);
                return;
              }

              const project = projects.find((proj) => proj.id === value);
              setCurrentProject(project ?? null);
            }}
          >
            <SelectTrigger className="w-[220px] bg-background">
              <SelectValue>
                {(value) => {
                  if (!value || value === ALL_PROJECTS_VALUE) {
                    return t("common:allProjects");
                  }

                  return (
                    projects.find((project) => project.id === value)?.name ??
                    t("common:allProjects")
                  );
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_PROJECTS_VALUE}>{t("common:allProjects")}</SelectItem>
              {projects.map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        <button
          onClick={toggleTheme}
          className="p-2 rounded-md hover:bg-accent transition-colors"
          title={
            (dark ? t("common:switchToLight") : t("common:switchToDark")) +
            " (" +
            shortcutDisplay(GLOBAL_SHORTCUTS[1]) +
            ")"
          }
        >
          {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
      </div>
    </header>
  );
}
