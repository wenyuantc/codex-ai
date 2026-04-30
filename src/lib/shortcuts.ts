export type ShortcutCategory = "navigation" | "global" | "page";

export interface ShortcutDef {
  id: string;
  keys: string;
  display: string;
  description: string;
  category: ShortcutCategory;
  page?: string;
}

export const NAV_SHORTCUTS: ShortcutDef[] = [
  { id: "nav-dashboard", keys: "meta+1", display: "⌘1", description: "仪表盘", category: "navigation", page: "/" },
  { id: "nav-projects", keys: "meta+2", display: "⌘2", description: "项目管理", category: "navigation", page: "/projects" },
  { id: "nav-kanban", keys: "meta+3", display: "⌘3", description: "看板管理", category: "navigation", page: "/kanban" },
  { id: "nav-sessions", keys: "meta+4", display: "⌘4", description: "对话管理", category: "navigation", page: "/sessions" },
  { id: "nav-employees", keys: "meta+5", display: "⌘5", description: "员工管理", category: "navigation", page: "/employees" },
  { id: "nav-settings", keys: "meta+6", display: "⌘6", description: "设置", category: "navigation", page: "/settings" },
];

export const GLOBAL_SHORTCUTS: ShortcutDef[] = [
  { id: "global-search", keys: "meta+k", display: "⌘K", description: "打开全局搜索", category: "global" },
  { id: "global-theme", keys: "meta+t", display: "⌘T", description: "切换主题", category: "global" },
  { id: "global-sidebar", keys: "meta+b", display: "⌘B", description: "切换侧边栏", category: "global" },
  { id: "global-help", keys: "?", display: "?", description: "显示快捷键帮助", category: "global" },
];

export const PAGE_SHORTCUTS: ShortcutDef[] = [
  { id: "page-kanban-new", keys: "n", display: "N", description: "新建任务", category: "page", page: "/kanban" },
  { id: "page-kanban-archive", keys: "a", display: "A", description: "归档管理", category: "page", page: "/kanban" },
  { id: "page-projects-new", keys: "n", display: "N", description: "新建项目", category: "page", page: "/projects" },
  { id: "page-employees-new", keys: "n", display: "N", description: "添加员工", category: "page", page: "/employees" },
  { id: "page-sessions-refresh", keys: "r", display: "R", description: "刷新对话列表", category: "page", page: "/sessions" },
];

export function isMac(): boolean {
  return navigator.platform.includes("Mac");
}

/** Convert canonical `meta` keys to platform-appropriate modifier */
export function shortcutKeys(def: ShortcutDef): string {
  if (isMac()) return def.keys;
  return def.keys.replace(/^meta(\+)/, "ctrl$1");
}

/** Convert canonical `⌘` display to platform-appropriate display */
export function shortcutDisplay(def: ShortcutDef): string {
  if (isMac()) return def.display;
  return def.display.replace(/^⌘/, "Ctrl+");
}

export function getAllShortcuts(): ShortcutDef[] {
  return [...NAV_SHORTCUTS, ...GLOBAL_SHORTCUTS, ...PAGE_SHORTCUTS];
}

export function getShortcutById(id: string): ShortcutDef | undefined {
  return getAllShortcuts().find((s) => s.id === id);
}
