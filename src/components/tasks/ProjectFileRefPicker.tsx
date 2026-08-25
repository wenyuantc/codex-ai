import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";

import { listProjectFiles } from "@/lib/backend";
import { isTauriRuntime } from "@/lib/taskAttachments";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

interface ProjectFileRefPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId: string | null;
  selectedPaths: string[];
  onConfirm: (paths: string[]) => void;
}

export function ProjectFileRefPicker({
  open,
  onOpenChange,
  projectId,
  selectedPaths,
  onConfirm,
}: ProjectFileRefPickerProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [paths, setPaths] = useState<string[]>([]);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQuery(query.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    if (!open) {
      return;
    }
    setQuery("");
    setDebouncedQuery("");
    setChecked(new Set(selectedPaths));
    setError(null);
  }, [open, selectedPaths]);

  useEffect(() => {
    if (!open || !projectId || !isTauriRuntime()) {
      setPaths([]);
      return;
    }

    let cancelled = false;
    setLoading(true);
    void listProjectFiles(projectId, debouncedQuery || null)
      .then((next) => {
        if (!cancelled) {
          setPaths(next);
          setError(null);
        }
      })
      .catch((loadError) => {
        if (!cancelled) {
          setPaths([]);
          setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open, projectId, debouncedQuery]);

  const selectedCount = useMemo(() => checked.size, [checked]);

  const togglePath = (path: string) => {
    setChecked((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[min(80vh,40rem)] sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("createDialog.selectProjectFilesTitle")}</DialogTitle>
          <p className="text-[11px] text-muted-foreground">
            {t("createDialog.selectProjectFilesHint")}
          </p>
        </DialogHeader>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("createDialog.selectProjectFilesSearch")}
          disabled={!projectId}
        />
        {error && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        <ScrollArea className="h-64 rounded-md border">
          {loading ? (
            <div className="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              {t("createDialog.selectProjectFilesLoading")}
            </div>
          ) : paths.length === 0 ? (
            <div className="px-3 py-8 text-center text-xs text-muted-foreground">
              {t("createDialog.selectProjectFilesEmpty")}
            </div>
          ) : (
            <ul className="divide-y">
              {paths.map((path) => (
                <li key={path}>
                  <label className="flex cursor-pointer items-start gap-2 px-3 py-2 text-xs hover:bg-muted/60">
                    <input
                      type="checkbox"
                      className="mt-0.5 h-3.5 w-3.5"
                      checked={checked.has(path)}
                      onChange={() => togglePath(path)}
                    />
                    <span className="break-all font-mono">{path}</span>
                  </label>
                </li>
              ))}
            </ul>
          )}
        </ScrollArea>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common:cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => {
              onConfirm(Array.from(checked));
              onOpenChange(false);
            }}
            disabled={!projectId}
          >
            {t("createDialog.selectProjectFilesConfirm")} ({selectedCount})
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
