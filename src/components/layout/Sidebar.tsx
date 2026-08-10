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
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";
import { GLOBAL_SHORTCUTS, NAV_SHORTCUTS, shortcutDisplay, shortcutKeys } from "@/lib/shortcuts";

const navItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "nav:dashboard" },
  { to: "/projects", icon: FolderKanban, labelKey: "nav:projects" },
  { to: "/kanban", icon: Columns3, labelKey: "nav:kanban" },
  { to: "/sessions", icon: MessagesSquare, labelKey: "nav:sessions" },
  { to: "/employees", icon: Users, labelKey: "nav:employees" },
  { to: "/settings", icon: Settings, labelKey: "nav:settings" },
  { to: "/trash", icon: Trash2, labelKey: "nav:trash" },
] as const;

export function Sidebar() {
  const { t } = useTranslation(["nav", "common"]);
  const [collapsed, setCollapsed] = useState(false);

  useHotkeys(shortcutKeys(GLOBAL_SHORTCUTS[2]), (e) => {
    e.preventDefault();
    setCollapsed((prev) => !prev);
  });

  const getShortcut = (to: string) => NAV_SHORTCUTS.find((s) => s.page === to);

  return (
    <aside
      className={cn(
        "flex flex-col h-screen bg-sidebar border-r border-sidebar-border transition-all duration-200",
        collapsed ? "w-16" : "w-56",
      )}
    >
      <div className="flex h-14 items-center gap-2 border-b border-sidebar-border px-4 text-sidebar-foreground">
        <Bot className="h-6 w-6 text-sidebar-primary shrink-0" />
        {!collapsed && (
          <span className="truncate text-sm font-semibold tracking-tight text-sidebar-foreground">
            {t("common:appName")}
          </span>
        )}
      </div>

      <nav className="flex-1 space-y-1 px-2 py-2">
        {navItems.map((item) => {
          const shortcut = getShortcut(item.to);
          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors",
                  isActive
                    ? "bg-sidebar-primary text-sidebar-primary-foreground font-medium"
                    : "text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
                )
              }
            >
              {({ isActive }) => (
                <>
                  <item.icon className="h-4 w-4 shrink-0" />
                  {!collapsed && (
                    <>
                      <span className="flex-1 truncate">{t(item.labelKey)}</span>
                      {shortcut && (
                        <Kbd
                          variant="subtle"
                          size="xs"
                          className={isActive ? "text-sidebar-primary-foreground" : ""}
                        >
                          {shortcutDisplay(shortcut)}
                        </Kbd>
                      )}
                    </>
                  )}
                </>
              )}
            </NavLink>
          );
        })}
      </nav>

      <div className="border-t border-sidebar-border p-2">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex w-full items-center justify-center rounded-md px-3 py-2 text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
        >
          {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
        </button>
      </div>
    </aside>
  );
}
