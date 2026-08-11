import { useEffect, useMemo, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { useTranslation } from "react-i18next";

import type { ProjectGitFileChangeRef, ProjectGitFilePreview } from "@/lib/types";
import { detectMonacoLanguage, getMonacoThemeName, loadMonaco } from "@/lib/monaco";
import i18n from "@/lib/i18n";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ProjectGitFilePreviewDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  preview: ProjectGitFilePreview | null;
  change: ProjectGitFileChangeRef | null;
  onOpenChange: (open: boolean) => void;
}

function getSnapshotStatusLabel(status: ProjectGitFilePreview["before_status"]) {
  switch (status) {
    case "text":
      return i18n.t("projects:snapshotStatusText");
    case "missing":
      return i18n.t("projects:snapshotStatusMissing");
    case "binary":
      return i18n.t("projects:snapshotStatusBinary");
    case "unavailable":
      return i18n.t("projects:snapshotStatusUnavailable");
    default:
      return status;
  }
}

function getSideLabel(side: "before" | "after") {
  return side === "before" ? i18n.t("projects:baselineVersion") : i18n.t("projects:workingVersion");
}

function getSnapshotDisplayText(
  side: "before" | "after",
  status: ProjectGitFilePreview["before_status"],
  text: string | null,
) {
  if (status === "text") {
    return text ?? "";
  }
  if (status === "missing") {
    return "";
  }
  if (status === "binary") {
    return i18n.t("projects:binaryPlaceholder", { side: getSideLabel(side) });
  }
  return i18n.t("projects:unavailablePlaceholder", { side: getSideLabel(side) });
}

export function ProjectGitFilePreviewDialog({
  open,
  loading,
  error,
  preview,
  change,
  onOpenChange,
}: ProjectGitFilePreviewDialogProps) {
  const { t } = useTranslation("projects");
  const editorContainerRef = useRef<HTMLDivElement | null>(null);
  const diffEditorRef = useRef<Monaco.editor.IStandaloneDiffEditor | null>(null);
  const originalModelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const modifiedModelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const [editorError, setEditorError] = useState<string | null>(null);

  const language = useMemo(
    () => detectMonacoLanguage(preview?.relative_path ?? change?.path ?? ""),
    [change?.path, preview?.relative_path],
  );

  useEffect(() => {
    if (!open || loading || error || editorError || !preview) {
      return;
    }

    let cancelled = false;

    void loadMonaco()
      .then((monaco) => {
        if (cancelled || !editorContainerRef.current) {
          return;
        }

        originalModelRef.current?.dispose();
        modifiedModelRef.current?.dispose();
        originalModelRef.current = monaco.editor.createModel(
          getSnapshotDisplayText("before", preview.before_status, preview.before_text),
          language,
        );
        modifiedModelRef.current = monaco.editor.createModel(
          getSnapshotDisplayText("after", preview.after_status, preview.after_text),
          language,
        );

        if (!diffEditorRef.current) {
          diffEditorRef.current = monaco.editor.createDiffEditor(editorContainerRef.current, {
            theme: getMonacoThemeName(),
            readOnly: true,
            originalEditable: false,
            automaticLayout: true,
            renderSideBySide: true,
            useInlineViewWhenSpaceIsLimited: false,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            wordWrap: "on",
            lineNumbersMinChars: 3,
            renderOverviewRuler: false,
            diffWordWrap: "on",
            ignoreTrimWhitespace: false,
            fontSize: 13,
          });
        }

        diffEditorRef.current.setModel({
          original: originalModelRef.current,
          modified: modifiedModelRef.current,
        });
        diffEditorRef.current.layout();

        resizeObserverRef.current?.disconnect();
        resizeObserverRef.current = new ResizeObserver(() => {
          diffEditorRef.current?.layout();
        });
        resizeObserverRef.current.observe(editorContainerRef.current);
      })
      .catch((loadError) => {
        if (!cancelled) {
          setEditorError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [editorError, error, language, loading, open, preview]);

  useEffect(() => {
    if (!open) {
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
      diffEditorRef.current?.dispose();
      diffEditorRef.current = null;
      originalModelRef.current?.dispose();
      originalModelRef.current = null;
      modifiedModelRef.current?.dispose();
      modifiedModelRef.current = null;
      setEditorError(null);
    }
  }, [open]);

  const titlePath = preview?.relative_path ?? change?.path ?? t("previewDefaultTitle");
  const message = error ?? editorError ?? preview?.message ?? null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]">
        <DialogHeader>
          <DialogTitle>{titlePath}</DialogTitle>
          <DialogDescription>
            {t("previewDescription", {
              before: preview?.before_label ?? t("beforeVersion"),
              after: preview?.after_label ?? t("afterVersion"),
            })}
          </DialogDescription>
        </DialogHeader>

        {change && (
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
            <div>{t("changeType", { type: change.change_type })}</div>
            {preview?.absolute_path && (
              <div className="mt-1 break-all font-mono">{preview.absolute_path}</div>
            )}
            {preview?.previous_path && (
              <div className="mt-1 break-all">
                {t("baselinePath", {
                  path: <span className="font-mono">{preview.previous_path}</span>,
                })}
              </div>
            )}
          </div>
        )}

        <div className="grid gap-2 md:grid-cols-2">
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
            <div className="font-medium text-foreground">
              {preview?.before_label ?? t("beforeVersion")}
            </div>
            <div className="mt-1 text-muted-foreground">
              {getSnapshotStatusLabel(preview?.before_status ?? "unavailable")}
              {preview?.before_truncated ? t("truncatedSuffix") : ""}
            </div>
            {preview?.previous_absolute_path && (
              <div className="mt-1 break-all font-mono text-muted-foreground">
                {preview.previous_absolute_path}
              </div>
            )}
          </div>
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
            <div className="font-medium text-foreground">
              {preview?.after_label ?? t("afterVersion")}
            </div>
            <div className="mt-1 text-muted-foreground">
              {getSnapshotStatusLabel(preview?.after_status ?? "unavailable")}
              {preview?.after_truncated ? t("truncatedSuffix") : ""}
            </div>
            {preview?.absolute_path && (
              <div className="mt-1 break-all font-mono text-muted-foreground">
                {preview.absolute_path}
              </div>
            )}
          </div>
        </div>

        {loading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-10 text-center text-sm text-muted-foreground">
            {t("loadingDiff")}
          </div>
        ) : message ? (
          <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
            {message}
          </div>
        ) : null}

        {preview ? (
          <div
            ref={editorContainerRef}
            className="h-[36rem] overflow-hidden rounded-md border border-border/70 bg-background"
          />
        ) : !loading && !message ? (
          <div className="rounded-md border border-dashed border-border px-3 py-10 text-center text-sm text-muted-foreground">
            {t("emptyDiff")}
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
