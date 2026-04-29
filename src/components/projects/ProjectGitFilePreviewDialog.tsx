import { useEffect, useMemo, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";

import type { ProjectGitFileChangeRef, ProjectGitFilePreview } from "@/lib/types";
import { detectMonacoLanguage, getMonacoThemeName, loadMonaco } from "@/lib/monaco";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";

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
      return "文本";
    case "missing":
      return "不存在";
    case "binary":
      return "二进制";
    case "unavailable":
      return "不可预览";
    default:
      return status;
  }
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
    return `/* ${side === "before" ? "基线版本" : "工作区版本"}是二进制文件，暂不支持文本 Diff 预览 */`;
  }
  return `/* ${side === "before" ? "基线版本" : "工作区版本"}暂不可预览 */`;
}

export function ProjectGitFilePreviewDialog({
  open,
  loading,
  error,
  preview,
  change,
  onOpenChange,
}: ProjectGitFilePreviewDialogProps) {
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

  const titlePath = preview?.relative_path ?? change?.path ?? "文件 Diff 预览";
  const message = error ?? editorError ?? preview?.message ?? null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]">
        <DialogHeader>
          <DialogTitle>{titlePath}</DialogTitle>
          <DialogDescription>
            使用 Monaco Diff Editor 对比 {preview?.before_label ?? "对比前版本"} 与 {preview?.after_label ?? "对比后版本"}。
          </DialogDescription>
        </DialogHeader>

        {change && (
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
            <div>变更类型：{change.change_type}</div>
            {preview?.absolute_path && (
              <div className="mt-1 break-all font-mono">{preview.absolute_path}</div>
            )}
            {preview?.previous_path && (
              <div className="mt-1 break-all">
                基线路径：<span className="font-mono">{preview.previous_path}</span>
              </div>
            )}
          </div>
        )}

        <div className="grid gap-2 md:grid-cols-2">
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
            <div className="font-medium text-foreground">{preview?.before_label ?? "对比前版本"}</div>
            <div className="mt-1 text-muted-foreground">
              {getSnapshotStatusLabel(preview?.before_status ?? "unavailable")}
              {preview?.before_truncated ? " · 已截断" : ""}
            </div>
            {preview?.previous_absolute_path && (
              <div className="mt-1 break-all font-mono text-muted-foreground">
                {preview.previous_absolute_path}
              </div>
            )}
          </div>
          <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
            <div className="font-medium text-foreground">{preview?.after_label ?? "对比后版本"}</div>
            <div className="mt-1 text-muted-foreground">
              {getSnapshotStatusLabel(preview?.after_status ?? "unavailable")}
              {preview?.after_truncated ? " · 已截断" : ""}
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
            正在加载 Diff 预览...
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
            暂无可展示的 Diff 内容。
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
