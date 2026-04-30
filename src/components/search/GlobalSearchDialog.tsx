import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";
import { GLOBAL_SHORTCUTS, shortcutDisplay, shortcutKeys } from "@/lib/shortcuts";
import {
  Bot,
  FolderKanban,
  Loader2,
  Search,
  TerminalSquare,
  UserRound,
} from "lucide-react";

import { searchGlobal } from "@/lib/backend";
import { execute } from "@/lib/database";
import type { GlobalSearchItem, GlobalSearchItemType, GlobalSearchResponse } from "@/lib/types";
import { useProjectStore } from "@/stores/projectStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

const TYPE_ORDER: GlobalSearchItemType[] = ["project", "task", "employee", "session"];
const SEARCH_DEBOUNCE_MS = 180;

const TYPE_LABELS: Record<GlobalSearchItemType, string> = {
  project: "项目",
  task: "任务",
  employee: "员工",
  session: "会话",
};

const TYPE_ICONS: Record<GlobalSearchItemType, typeof FolderKanban> = {
  project: FolderKanban,
  task: Bot,
  employee: UserRound,
  session: TerminalSquare,
};

function buildNavigationLogDetails(item: GlobalSearchItem) {
  return `${TYPE_LABELS[item.item_type]}：${item.title}`;
}

async function recordGlobalSearchNavigation(item: GlobalSearchItem) {
  try {
    await execute(
      "INSERT INTO activity_logs (id, employee_id, action, details, task_id, project_id, created_at) VALUES (?1, NULL, ?2, ?3, ?4, ?5, datetime('now'))",
      [
        globalThis.crypto?.randomUUID?.() ?? `search-${Date.now()}`,
        "global_search_navigated",
        buildNavigationLogDetails(item),
        item.task_id,
        item.project_id,
      ],
    );
  } catch (error) {
    console.error("Failed to record global search navigation:", error);
  }
}

export function GlobalSearchDialog() {
  const navigate = useNavigate();
  const location = useLocation();
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const allProjects = useProjectStore((state) => state.allProjects);
  const setCurrentProject = useProjectStore((state) => state.setCurrentProject);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeType, setActiveType] = useState<GlobalSearchItemType | "all">("all");
  const [response, setResponse] = useState<GlobalSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const requestIdRef = useRef(0);

  useHotkeys(shortcutKeys(GLOBAL_SHORTCUTS[0]), (e) => {
    e.preventDefault();
    setOpen(true);
  });

  useEffect(() => {
    if (!open) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 20);

    return () => window.clearTimeout(timeoutId);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    if (!query.trim()) {
      setLoading(false);
      setError(null);
      setResponse(null);
      return;
    }

    const currentRequestId = requestIdRef.current + 1;
    requestIdRef.current = currentRequestId;
    setLoading(true);
    setError(null);

    const timeoutId = window.setTimeout(() => {
      void searchGlobal({
        query,
        environment_mode: environmentMode,
        limit: activeType === "all" ? 24 : 20,
        types: activeType === "all" ? undefined : [activeType],
      })
        .then((nextResponse) => {
          if (requestIdRef.current !== currentRequestId) {
            return;
          }
          setResponse(nextResponse);
        })
        .catch((nextError) => {
          if (requestIdRef.current !== currentRequestId) {
            return;
          }
          setError(nextError instanceof Error ? nextError.message : "全局搜索失败");
          setResponse(null);
        })
        .finally(() => {
          if (requestIdRef.current === currentRequestId) {
            setLoading(false);
          }
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timeoutId);
  }, [activeType, environmentMode, open, query]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setActiveIndex(0);
  }, [activeType, open, query, response?.items]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const activeElement = document.querySelector<HTMLElement>(`[data-search-index="${activeIndex}"]`);
    activeElement?.scrollIntoView({ block: "nearest" });
  }, [activeIndex, open, response]);

  useEffect(() => {
    if (open) {
      setOpen(false);
    }
  }, [location.pathname, location.search]);

  const filteredItems = useMemo(() => {
    if (!response) {
      return [];
    }

    return activeType === "all"
      ? response.items
      : response.items.filter((item) => item.item_type === activeType);
  }, [activeType, response]);

  const groupedItems = useMemo(() => {
    const grouped = new Map<GlobalSearchItemType, GlobalSearchItem[]>();
    for (const type of TYPE_ORDER) {
      grouped.set(type, []);
    }

    for (const item of filteredItems) {
      grouped.get(item.item_type)?.push(item);
    }

    return grouped;
  }, [filteredItems]);

  const visibleItems = useMemo(() => {
    if (activeType !== "all") {
      return filteredItems;
    }

    return TYPE_ORDER.flatMap((type) => groupedItems.get(type) ?? []);
  }, [activeType, filteredItems, groupedItems]);

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);

    if (!nextOpen) {
      requestIdRef.current += 1;
      setQuery("");
      setActiveType("all");
      setResponse(null);
      setError(null);
      setLoading(false);
      setActiveIndex(0);
    }
  };

  const handleNavigate = async (item: GlobalSearchItem) => {
    const targetProject = item.project_id
      ? allProjects.find((project) => project.id === item.project_id) ?? null
      : null;

    if (item.item_type === "project") {
      setCurrentProject(targetProject);
    } else if (item.item_type === "task" || item.item_type === "employee") {
      setCurrentProject(targetProject);
    }

    await recordGlobalSearchNavigation(item);
    handleOpenChange(false);
    navigate(item.navigation_path, {
      state: {
        globalSearchNonce: Date.now(),
      },
    });
  };

  const handleInputKeyDown = async (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (visibleItems.length === 0) {
        return;
      }
      setActiveIndex((current) => Math.min(visibleItems.length - 1, current + 1));
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => Math.max(0, current - 1));
      return;
    }

    if (event.key === "Enter" && visibleItems[activeIndex]) {
      event.preventDefault();
      await handleNavigate(visibleItems[activeIndex]);
    }
  };

  return (
    <>
      <Button
        type="button"
        variant="outline"
        className="h-9 min-w-[12rem] justify-between gap-3 px-3 text-muted-foreground"
        onClick={() => setOpen(true)}
      >
        <span className="flex items-center gap-2">
          <Search className="h-4 w-4" />
          全局搜索
        </span>
        <span className="rounded border border-border bg-background px-1.5 py-0.5 text-[11px] text-muted-foreground/80">
          {shortcutDisplay(GLOBAL_SHORTCUTS[0])}
        </span>
      </Button>

      <Dialog open={open} onOpenChange={handleOpenChange}>
        <DialogContent className="max-w-3xl gap-0 overflow-hidden p-0">
          <DialogHeader className="border-b border-border/60 px-5 py-4">
            <DialogTitle>全局搜索</DialogTitle>
          </DialogHeader>

          <div className="space-y-4 px-5 py-4">
            <Input
              ref={inputRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={handleInputKeyDown}
              placeholder="搜索项目、任务、员工、会话"
            />

            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                size="sm"
                variant={activeType === "all" ? "default" : "outline"}
                onClick={() => setActiveType("all")}
              >
                全部
              </Button>
              {TYPE_ORDER.map((type) => (
                <Button
                  key={type}
                  type="button"
                  size="sm"
                  variant={activeType === type ? "default" : "outline"}
                  onClick={() => setActiveType(type)}
                >
                  {TYPE_LABELS[type]}
                </Button>
              ))}
              {response?.state === "ok" && (
                <span className="ml-auto text-xs text-muted-foreground">
                  共 {response.total} 条匹配结果
                </span>
              )}
            </div>
          </div>

          <ScrollArea className="max-h-[28rem] border-t border-border/60">
            <div className="px-5 py-4">
              {error ? (
                <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-8 text-center text-sm text-destructive">
                  {error}
                </div>
              ) : loading ? (
                <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  正在搜索...
                </div>
              ) : !query.trim() ? (
                <div className="space-y-2 py-6 text-sm text-muted-foreground">
                  <p>从一个入口搜索项目、任务、员工和会话。</p>
                  <p>支持鼠标点击，也支持上下方向键选择后按 Enter 直接跳转。</p>
                </div>
              ) : response?.state !== "ok" ? (
                <div className="py-8 text-center text-sm text-muted-foreground">
                  {response?.message ?? "请输入关键词后开始搜索。"}
                </div>
              ) : visibleItems.length === 0 ? (
                <div className="py-8 text-center text-sm text-muted-foreground">
                  没有找到匹配的对象
                </div>
              ) : activeType === "all" ? (
                <div className="space-y-5">
                  {TYPE_ORDER.map((type) => {
                    const items = groupedItems.get(type) ?? [];
                    if (items.length === 0) {
                      return null;
                    }

                    return (
                      <section key={type} className="space-y-2">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <Badge variant="outline">{TYPE_LABELS[type]}</Badge>
                            <span className="text-xs text-muted-foreground">{items.length} 条</span>
                          </div>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => setActiveType(type)}
                          >
                            查看全部
                          </Button>
                        </div>
                        <div className="space-y-2">
                          {items.map((item) => {
                            const index = visibleItems.findIndex((candidate) => candidate === item);
                            return (
                              <SearchResultRow
                                key={`${item.item_type}-${item.item_id}`}
                                item={item}
                                active={index === activeIndex}
                                index={index}
                                onSelect={handleNavigate}
                              />
                            );
                          })}
                        </div>
                      </section>
                    );
                  })}
                </div>
              ) : (
                <div className="space-y-2">
                  {visibleItems.map((item, index) => (
                    <SearchResultRow
                      key={`${item.item_type}-${item.item_id}`}
                      item={item}
                      active={index === activeIndex}
                      index={index}
                      onSelect={handleNavigate}
                    />
                  ))}
                </div>
              )}
            </div>
          </ScrollArea>
        </DialogContent>
      </Dialog>
    </>
  );
}

interface SearchResultRowProps {
  item: GlobalSearchItem;
  active: boolean;
  index: number;
  onSelect: (item: GlobalSearchItem) => Promise<void>;
}

function SearchResultRow({ item, active, index, onSelect }: SearchResultRowProps) {
  const Icon = TYPE_ICONS[item.item_type];

  return (
    <button
      type="button"
      data-search-index={index}
      className={`flex w-full items-start gap-3 rounded-xl border px-3 py-3 text-left transition-colors ${
        active
          ? "border-primary bg-primary/5"
          : "border-border/60 hover:border-border hover:bg-accent/40"
      }`}
      onClick={() => void onSelect(item)}
    >
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        <Icon className="h-4 w-4" />
      </div>

      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium text-foreground">{item.title}</span>
          <Badge variant="outline">{TYPE_LABELS[item.item_type]}</Badge>
        </div>
        {item.subtitle && (
          <p className="truncate text-xs text-muted-foreground">{item.subtitle}</p>
        )}
        {item.summary && (
          <p className="line-clamp-2 text-sm text-muted-foreground">{item.summary}</p>
        )}
      </div>
    </button>
  );
}
