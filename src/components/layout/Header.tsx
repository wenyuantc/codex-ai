import { useLocation, useNavigate } from "react-router-dom";
import { Laptop, Moon, ServerCog, Sun } from "lucide-react";
import { useEffect, useState } from "react";
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

const ALL_PROJECTS_VALUE = "__all_projects__";

const pageTitles: Record<string, string> = {
  "/": "仪表盘",
  "/projects": "项目管理",
  "/kanban": "看板管理",
  "/employees": "员工管理",
  "/sessions": "Session 管理",
  "/settings": "系统设置",
};

export function Header() {
  const location = useLocation();
  const navigate = useNavigate();
  const title = pageTitles[location.pathname] || "AI员工协作系统";
  const {
    projects,
    currentProject,
    environmentMode,
    sshConfigs,
    setCurrentProject,
    setEnvironmentMode,
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
    return () => window.removeEventListener(THEME_CHANGE_EVENT, handleThemeChange);
  }, []);

  useEffect(() => {
    if (environmentMode === "ssh" && sshConfigs.length === 0 && location.pathname !== "/settings") {
      navigate("/settings");
    }
  }, [environmentMode, location.pathname, navigate, sshConfigs.length]);

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
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-muted-foreground">{getEnvironmentModeLabel(environmentMode)}</span>
          <div className="inline-flex rounded-md border border-border bg-muted/30 p-0.5">
            <Button
              type="button"
              variant={environmentMode === "local" ? "default" : "ghost"}
              size="sm"
              className="h-7 gap-1.5 px-2 text-xs"
              onClick={() => void handleEnvironmentModeChange("local")}
            >
              <Laptop className="h-3.5 w-3.5" />
              本地
            </Button>
            <Button
              type="button"
              variant={environmentMode === "ssh" ? "default" : "ghost"}
              size="sm"
              className="h-7 gap-1.5 px-2 text-xs"
              onClick={() => void handleEnvironmentModeChange("ssh")}
            >
              <ServerCog className="h-3.5 w-3.5" />
              SSH
            </Button>
          </div>
        </div>
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
                    return "全部项目";
                  }

                  return projects.find((project) => project.id === value)?.name ?? "全部项目";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_PROJECTS_VALUE}>全部项目</SelectItem>
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
          title={dark ? "切换亮色模式" : "切换暗色模式"}
        >
          {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
      </div>
    </header>
  );
}
