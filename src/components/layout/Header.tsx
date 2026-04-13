import { useLocation } from "react-router-dom";
import { Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const ALL_PROJECTS_VALUE = "__all_projects__";

const pageTitles: Record<string, string> = {
  "/": "仪表盘",
  "/projects": "项目管理",
  "/kanban": "任务看板",
  "/employees": "员工管理",
  "/settings": "系统设置",
};

function getThemePreference(): boolean {
  const stored = localStorage.getItem("theme");
  if (stored) return stored === "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function Header() {
  const location = useLocation();
  const title = pageTitles[location.pathname] || "AI员工协作系统";
  const { projects, currentProject, setCurrentProject, fetchProjects } = useProjectStore();
  const [dark, setDark] = useState(false);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  useEffect(() => {
    const isDark = getThemePreference();
    setDark(isDark);
    document.documentElement.classList.toggle("dark", isDark);
  }, []);

  const toggleTheme = () => {
    const next = !dark;
    setDark(next);
    document.documentElement.classList.toggle("dark", next);
    localStorage.setItem("theme", next ? "dark" : "light");
  };

  return (
    <header className="flex items-center justify-between h-14 px-6 border-b border-border bg-background">
      <h1 className="text-lg font-semibold">{title}</h1>

      <div className="flex items-center gap-4">
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
