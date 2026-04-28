import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";

import { loadMonaco } from "@/lib/monaco";
import { cn } from "@/lib/utils";

interface MonacoMarkdownEditorProps {
  value: string;
  onChange?: (value: string) => void;
  onBlur?: () => void;
  readOnly?: boolean;
  placeholder?: string;
  className?: string;
}

export function MonacoMarkdownEditor({
  value,
  onChange,
  onBlur,
  readOnly = false,
  placeholder,
  className,
}: MonacoMarkdownEditorProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const onChangeRef = useRef(onChange);
  const onBlurRef = useRef(onBlur);
  const valueRef = useRef(value);
  const readOnlyRef = useRef(readOnly);
  const [editorError, setEditorError] = useState<string | null>(null);

  onChangeRef.current = onChange;
  onBlurRef.current = onBlur;
  valueRef.current = value;
  readOnlyRef.current = readOnly;

  useEffect(() => {
    let cancelled = false;

    void loadMonaco()
      .then((monaco) => {
        if (cancelled || !containerRef.current) {
          return;
        }

        modelRef.current = monaco.editor.createModel(valueRef.current, "markdown");
        editorRef.current = monaco.editor.create(containerRef.current, {
          model: modelRef.current,
          readOnly: readOnlyRef.current,
          automaticLayout: true,
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          wordWrap: "on",
          lineNumbers: "off",
          folding: false,
          glyphMargin: false,
          lineDecorationsWidth: 8,
          lineNumbersMinChars: 0,
          renderLineHighlight: readOnlyRef.current ? "none" : "line",
          overviewRulerLanes: 0,
          fontSize: 13,
          padding: { top: 10, bottom: 10 },
          scrollbar: {
            verticalScrollbarSize: 8,
            horizontalScrollbarSize: 8,
          },
        });

        editorRef.current.onDidChangeModelContent(() => {
          onChangeRef.current?.(editorRef.current?.getValue() ?? "");
        });
        editorRef.current.onDidBlurEditorWidget(() => {
          onBlurRef.current?.();
        });

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
    const position = editor.getPosition();
    editor.setValue(value);
    if (position) {
      editor.setPosition(position);
    }
  }, [value]);

  useEffect(() => {
    editorRef.current?.updateOptions({
      readOnly,
      renderLineHighlight: readOnly ? "none" : "line",
    });
  }, [readOnly]);

  if (editorError) {
    return (
      <div className={cn("rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive", className)}>
        Monaco 加载失败：{editorError}
      </div>
    );
  }

  return (
    <div
      className={cn("relative overflow-hidden rounded-md border border-border bg-background", className)}
      data-placeholder={placeholder}
    >
      <div ref={containerRef} className="h-full min-h-[inherit]" />
      {!value.trim() && placeholder && (
        <div className="pointer-events-none absolute left-3 top-2.5 text-xs text-muted-foreground">
          {placeholder}
        </div>
      )}
    </div>
  );
}
