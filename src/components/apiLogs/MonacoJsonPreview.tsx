import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { useTranslation } from "react-i18next";

import { getMonacoThemeName, loadMonaco } from "@/lib/monaco";
import { cn } from "@/lib/utils";

interface MonacoJsonPreviewProps {
  value: string;
  className?: string;
}

export function MonacoJsonPreview({ value, className }: MonacoJsonPreviewProps) {
  const { t } = useTranslation("apiLogs");
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const valueRef = useRef(value);
  const [editorError, setEditorError] = useState<string | null>(null);

  valueRef.current = value;

  useEffect(() => {
    let cancelled = false;

    void loadMonaco()
      .then((monaco) => {
        if (cancelled || !containerRef.current) {
          return;
        }

        const model = monaco.editor.createModel(valueRef.current, "json");
        const editor = monaco.editor.create(containerRef.current, {
          model,
          theme: getMonacoThemeName(),
          readOnly: true,
          automaticLayout: true,
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          wordWrap: "on",
          lineNumbers: "on",
          folding: true,
          glyphMargin: false,
          renderLineHighlight: "none",
          overviewRulerLanes: 0,
          fontSize: 13,
          padding: { top: 10, bottom: 10 },
          scrollbar: {
            verticalScrollbarSize: 8,
            horizontalScrollbarSize: 8,
          },
          contextmenu: false,
          domReadOnly: true,
        });
        if (cancelled) {
          editor.dispose();
          model.dispose();
          return;
        }

        modelRef.current = model;
        editorRef.current = editor;
        if (model.getValue() !== valueRef.current) {
          model.setValue(valueRef.current);
        }

        resizeObserverRef.current?.disconnect();
        resizeObserverRef.current = new ResizeObserver(() => {
          editorRef.current?.layout();
        });
        resizeObserverRef.current.observe(containerRef.current);
      })
      .catch((error) => {
        if (!cancelled) {
          setEditorError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
      editorRef.current?.dispose();
      editorRef.current = null;
      modelRef.current?.dispose();
      modelRef.current = null;
    };
  }, []);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || editor.getValue() === value) {
      return;
    }
    editor.setValue(value);
  }, [value]);

  if (editorError) {
    return (
      <div
        className={cn(
          "rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive",
          className,
        )}
      >
        {t("monacoLoadFailed", { error: editorError })}
      </div>
    );
  }

  return (
    <div className={cn("overflow-hidden rounded-md border border-border bg-background", className)}>
      <div ref={containerRef} className="h-full min-h-[inherit]" />
    </div>
  );
}
