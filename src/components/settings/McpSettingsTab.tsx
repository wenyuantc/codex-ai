import { useEffect, useState } from "react";
import { Loader2, Plus, RefreshCw, Save, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  exportMcpServersSnippet,
  getMcpServers,
  resetMcpServers,
  updateMcpServers,
  type McpServerConfig,
} from "@/lib/backend";

function createEmptyServer(): McpServerConfig {
  return {
    id: globalThis.crypto?.randomUUID?.() ?? `mcp-${Date.now()}`,
    name: "",
    command: "",
    args: [],
    env: [],
    enabled: false,
    notes: "",
  };
}

export function McpSettingsTab() {
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [snippet, setSnippet] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const doc = await getMcpServers();
      setServers(doc.servers);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const updateServer = (id: string, patch: Partial<McpServerConfig>) => {
    setServers((current) =>
      current.map((server) => (server.id === id ? { ...server, ...patch } : server)),
    );
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const doc = await updateMcpServers(servers);
      setServers(doc.servers);
      setMessage("MCP 配置已保存（应用配置目录 mcp-servers.json）。");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const doc = await resetMcpServers();
      setServers(doc.servers);
      setMessage("已重置为默认示例配置。");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleExport = async () => {
    setError(null);
    try {
      const text = await exportMcpServersSnippet();
      setSnippet(text);
      await navigator.clipboard?.writeText(text);
      setMessage("已生成导出片段，并尝试复制到剪贴板。");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        加载 MCP 配置…
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-border bg-card p-4 space-y-2">
        <h3 className="text-sm font-medium">MCP 服务器管理</h3>
        <p className="text-xs text-muted-foreground">
          在此维护 MCP Server 清单，可导出为 JSON 片段合并到 Codex/Claude 等引擎配置。
          配置保存在应用配置目录（不在 SQL 备份范围内）。
        </p>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => setServers((c) => [...c, createEmptyServer()])}>
            <Plus className="h-4 w-4" />
            添加服务器
          </Button>
          <Button size="sm" variant="outline" onClick={() => void handleSave()} disabled={saving}>
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            保存
          </Button>
          <Button size="sm" variant="outline" onClick={() => void handleExport()}>
            导出片段
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void handleReset()} disabled={saving}>
            <RefreshCw className="h-4 w-4" />
            重置示例
          </Button>
        </div>
        {message && <p className="text-xs text-green-700 dark:text-green-300">{message}</p>}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>

      <div className="space-y-3">
        {servers.length === 0 && (
          <p className="text-sm text-muted-foreground">暂无 MCP 服务器，请点击「添加服务器」。</p>
        )}
        {servers.map((server) => (
          <div key={server.id} className="space-y-3 rounded-lg border border-border bg-card p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={server.enabled}
                  onChange={(e) => updateServer(server.id, { enabled: e.target.checked })}
                />
                启用
              </label>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setServers((c) => c.filter((s) => s.id !== server.id))}
              >
                <Trash2 className="h-4 w-4" />
                删除
              </Button>
            </div>
            <div className="grid gap-2 sm:grid-cols-2">
              <Input
                placeholder="名称"
                value={server.name}
                onChange={(e) => updateServer(server.id, { name: e.target.value })}
              />
              <Input
                placeholder="启动命令（如 npx / uvx / 绝对路径）"
                value={server.command}
                onChange={(e) => updateServer(server.id, { command: e.target.value })}
              />
            </div>
            <Input
              placeholder="参数（空格分隔）"
              value={server.args.join(" ")}
              onChange={(e) =>
                updateServer(server.id, {
                  args: e.target.value
                    .split(/\s+/)
                    .map((part) => part.trim())
                    .filter(Boolean),
                })
              }
            />
            <Textarea
              placeholder="备注（可选）"
              value={server.notes ?? ""}
              onChange={(e) => updateServer(server.id, { notes: e.target.value })}
              rows={2}
            />
          </div>
        ))}
      </div>

      {snippet && (
        <div className="rounded-lg border border-border bg-muted/30 p-3">
          <p className="mb-2 text-xs font-medium">导出片段预览</p>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap text-[11px] font-mono">
            {snippet}
          </pre>
        </div>
      )}
    </div>
  );
}
