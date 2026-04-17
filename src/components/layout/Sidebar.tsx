import { useState } from "react";
import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  FolderKanban,
  Columns3,
  Users,
  Settings,
  MessagesSquare,
  ChevronLeft,
  ChevronRight,
  Bot,
} from "lucide-react";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "仪表盘" },
  { to: "/projects", icon: FolderKanban, label: "项目管理" },
  { to: "/kanban", icon: Columns3, label: "看板管理" },
  { to: "/sessions", icon: MessagesSquare, label: "Session 管理" },
  { to: "/employees", icon: Users, label: "员工管理" },
  { to: "/settings", icon: Settings, label: "设置" },
];

export function Sidebar() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <aside
      className={cn(
        "flex flex-col h-screen bg-sidebar-background border-r border-sidebar-border transition-all duration-200",
        collapsed ? "w-16" : "w-56"
      )}
    >
      <div className="flex h-14 items-center gap-2 border-b border-black/8 px-4 text-zinc-900 dark:border-white/10 dark:text-white">
        <Bot className="h-6 w-6 text-sidebar-primary shrink-0" />
        {!collapsed && (
          <span className="truncate text-sm font-semibold tracking-tight text-zinc-900 dark:text-white">
            AI员工协作系统
          </span>
        )}
      </div>

      <nav className="flex-1 space-y-1 px-2 py-2">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
                isActive
                  ? "bg-zinc-900 text-white font-medium dark:bg-white dark:text-zinc-900"
                  : "text-zinc-600 hover:bg-black/5 hover:text-zinc-900 dark:text-white/72 dark:hover:bg-white/8 dark:hover:text-white"
              )
            }
          >
            <item.icon className="h-4 w-4 shrink-0" />
            {!collapsed && <span>{item.label}</span>}
          </NavLink>
        ))}
      </nav>

      <div className="border-t border-black/8 p-2 dark:border-white/10">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex w-full items-center justify-center rounded-md px-3 py-2 text-zinc-600 transition-colors hover:bg-black/5 hover:text-zinc-900 dark:text-white/72 dark:hover:bg-white/8 dark:hover:text-white"
        >
          {collapsed ? (
            <ChevronRight className="h-4 w-4" />
          ) : (
            <ChevronLeft className="h-4 w-4" />
          )}
        </button>
      </div>
    </aside>
  );
}
