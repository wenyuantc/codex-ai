import { useEffect, useState } from "react";
import { Loader2, Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";

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

const EXAMPLE_FILESYSTEM_ID = "example-filesystem";

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

function localizeExampleServers(
  servers: McpServerConfig[],
  t: (key: string) => string,
): McpServerConfig[] {
  return servers.map((server) => {
    if (server.id !== EXAMPLE_FILESYSTEM_ID) {
      return server;
    }
    return {
      ...server,
      name: t("mcp.exampleFilesystem.name"),
      notes: t("mcp.exampleFilesystem.notes"),
    };
  });
}

export function McpSettingsTab() {
  const { t } = useTranslation("settings");
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
      setServers(localizeExampleServers(doc.servers, t));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    // Reload when locale changes so example name/notes follow UI language.
  }, [t]);

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
      setServers(localizeExampleServers(doc.servers, t));
      setMessage(t("mcp.messages.saved"));
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
      setServers(localizeExampleServers(doc.servers, t));
      setMessage(t("mcp.messages.reset"));
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
      setMessage(t("mcp.messages.exported"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        {t("mcp.states.loading")}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-border bg-card p-4 space-y-2">
        <h3 className="text-sm font-medium">{t("mcp.title")}</h3>
        <p className="text-xs text-muted-foreground">{t("mcp.description")}</p>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => setServers((c) => [...c, createEmptyServer()])}>
            <Plus className="h-4 w-4" />
            {t("mcp.actions.addServer")}
          </Button>
          <Button size="sm" variant="outline" onClick={() => void handleSave()} disabled={saving}>
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            {t("mcp.actions.save")}
          </Button>
          <Button size="sm" variant="outline" onClick={() => void handleExport()}>
            {t("mcp.actions.exportSnippet")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void handleReset()} disabled={saving}>
            <RefreshCw className="h-4 w-4" />
            {t("mcp.actions.resetExample")}
          </Button>
        </div>
        {message && <p className="text-xs text-green-700 dark:text-green-300">{message}</p>}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>

      <div className="space-y-3">
        {servers.length === 0 && (
          <p className="text-sm text-muted-foreground">{t("mcp.states.empty")}</p>
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
                {t("mcp.fields.enabled")}
              </label>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setServers((c) => c.filter((s) => s.id !== server.id))}
              >
                <Trash2 className="h-4 w-4" />
                {t("mcp.actions.delete")}
              </Button>
            </div>
            <div className="grid gap-2 sm:grid-cols-2">
              <Input
                placeholder={t("mcp.fields.namePlaceholder")}
                value={server.name}
                onChange={(e) => updateServer(server.id, { name: e.target.value })}
              />
              <Input
                placeholder={t("mcp.fields.commandPlaceholder")}
                value={server.command}
                onChange={(e) => updateServer(server.id, { command: e.target.value })}
              />
            </div>
            <Input
              placeholder={t("mcp.fields.argsPlaceholder")}
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
              placeholder={t("mcp.fields.notesPlaceholder")}
              value={server.notes ?? ""}
              onChange={(e) => updateServer(server.id, { notes: e.target.value })}
              rows={2}
            />
          </div>
        ))}
      </div>

      {snippet && (
        <div className="rounded-lg border border-border bg-muted/30 p-3">
          <p className="mb-2 text-xs font-medium">{t("mcp.preview.title")}</p>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap text-[11px] font-mono">
            {snippet}
          </pre>
        </div>
      )}
    </div>
  );
}
