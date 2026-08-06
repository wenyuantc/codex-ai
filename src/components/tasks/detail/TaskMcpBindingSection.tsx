import { useCallback, useEffect, useState } from "react";
import { Loader2, Plug, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  getMcpServers,
  getTaskMcpBinding,
  setTaskMcpBinding,
  type McpServerConfig,
  type TaskMcpBindingMode,
} from "@/lib/backend";

interface TaskMcpBindingSectionProps {
  taskId: string;
}

export function TaskMcpBindingSection({ taskId }: TaskMcpBindingSectionProps) {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [catalog, setCatalog] = useState<McpServerConfig[]>([]);
  const [mode, setMode] = useState<TaskMcpBindingMode>("inherit");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [effectiveNames, setEffectiveNames] = useState<string[]>([]);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [doc, binding] = await Promise.all([getMcpServers(), getTaskMcpBinding(taskId)]);
      setCatalog(doc.servers);
      setMode(binding.mode);
      setSelectedIds(binding.server_ids);
      setEffectiveNames(binding.effective.map((server) => server.name));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [taskId]);

  useEffect(() => {
    void load();
  }, [load]);

  // Live preview of what would take effect after save (does not require server round-trip).
  useEffect(() => {
    if (mode === "inherit") {
      setEffectiveNames(catalog.filter((server) => server.enabled).map((server) => server.name));
      return;
    }
    setEffectiveNames(
      catalog
        .filter((server) => selectedIds.includes(server.id))
        .map((server) => server.name),
    );
  }, [mode, selectedIds, catalog]);

  const toggleId = (id: string) => {
    setSelectedIds((current) =>
      current.includes(id) ? current.filter((value) => value !== id) : [...current, id],
    );
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const binding = await setTaskMcpBinding({
        taskId,
        mode,
        serverIds: mode === "override" ? selectedIds : [],
      });
      setMode(binding.mode);
      setSelectedIds(binding.server_ids);
      setEffectiveNames(binding.effective.map((server) => server.name));
      setMessage(
        binding.mode === "inherit"
          ? "已保存：继承全局默认 MCP"
          : binding.server_ids.length === 0
            ? "已保存：本任务显式禁用全部 MCP"
            : `已保存：本任务启用 ${binding.effective.length} 个 MCP`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <section className="rounded-md border border-border bg-muted/20 p-3">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          加载任务 MCP 绑定…
        </div>
      </section>
    );
  }

  return (
    <section className="rounded-md border border-border bg-muted/20 p-3 space-y-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="space-y-1">
          <div className="flex items-center gap-2 text-sm font-medium text-foreground">
            <Plug className="h-4 w-4 text-primary" />
            MCP 工具绑定
          </div>
          <p className="text-[11px] text-muted-foreground">
            控制本任务会话启用的 MCP 服务器。默认继承设置页中全局已启用的服务器；关闭继承后可多选或全不选（显式空集）。
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={() => void handleSave()} disabled={saving}>
          {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
          保存绑定
        </Button>
      </div>

      <label className="flex items-center gap-2 text-xs text-foreground">
        <input
          type="checkbox"
          className="h-3.5 w-3.5 rounded border-input"
          checked={mode === "inherit"}
          onChange={(event) => {
            const inherit = event.target.checked;
            setMode(inherit ? "inherit" : "override");
            if (!inherit && selectedIds.length === 0) {
              // Seed override from currently enabled globals for easier edit.
              setSelectedIds(catalog.filter((server) => server.enabled).map((server) => server.id));
            }
          }}
        />
        使用全局默认（设置页中 enabled 的服务器）
      </label>

      {mode === "override" && (
        <div className="space-y-2 rounded-md border border-border bg-background/70 p-2">
          {catalog.length === 0 ? (
            <p className="text-[11px] text-muted-foreground">
              尚未配置 MCP 服务器，请先到「设置 → MCP 管理」添加。
            </p>
          ) : (
            catalog.map((server) => (
              <label
                key={server.id}
                className="flex items-start gap-2 text-xs text-foreground"
              >
                <input
                  type="checkbox"
                  className="mt-0.5 h-3.5 w-3.5 rounded border-input"
                  checked={selectedIds.includes(server.id)}
                  onChange={() => toggleId(server.id)}
                />
                <span className="min-w-0">
                  <span className="font-medium">{server.name}</span>
                  {!server.enabled && (
                    <span className="ml-1 text-[10px] text-muted-foreground">（全局未默认启用）</span>
                  )}
                  <span className="block truncate text-[10px] text-muted-foreground">
                    {server.command} {server.args.join(" ")}
                  </span>
                </span>
              </label>
            ))
          )}
          <p className="text-[10px] text-muted-foreground">
            全不选表示本任务显式禁用全部 MCP（与「继承全局」不同）。
          </p>
        </div>
      )}

      <div className="rounded-md border border-border bg-background/50 px-2 py-1.5 text-[11px] text-muted-foreground">
        当前将生效：
        {effectiveNames.length === 0
          ? "（无 MCP）"
          : effectiveNames.map((name) => (
              <span
                key={name}
                className="ml-1 inline-flex rounded bg-muted px-1.5 py-0.5 text-[10px] text-foreground"
              >
                {name}
              </span>
            ))}
      </div>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}
      {message && !error && (
        <div className="rounded-md border border-border bg-background/70 px-3 py-2 text-xs text-muted-foreground">
          {message}
        </div>
      )}
    </section>
  );
}
