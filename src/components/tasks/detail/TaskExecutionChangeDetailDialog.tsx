import { useEffect, useMemo, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { useTranslation } from "react-i18next";

import type { CodexSessionFileChangeDetail } from "@/lib/types";
import { detectMonacoLanguage, getMonacoThemeName, loadMonaco } from "@/lib/monaco";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  getExecutionChangeCaptureModeDescription,
  getExecutionChangeCaptureModeLabel,
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getExecutionSnapshotStatusLabel,
} from "./taskDetailViewHelpers";

interface TaskExecutionChangeDetailDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  detail: CodexSessionFileChangeDetail | null;
  revealLine?: number | null;
  findingMessage?: string | null;
  onOpenChange: (open: boolean) => void;
}

function SnapshotMeta({
  label,
  status,
  truncated,
}: {
  label: string;
  status: "text" | "missing" | "binary" | "unavailable";
  truncated: boolean;
}) {
  const { t } = useTranslation("tasks");
  return (
    <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs">
      <div className="font-medium text-foreground">{label}</div>
      <div className="mt-1 text-muted-foreground">
        {getExecutionSnapshotStatusLabel(status)}
        {truncated ? ` ${t("detail.labels.truncatedSuffix")}` : ""}
      </div>
    </div>
  );
}

export function TaskExecutionChangeDetailDialog({
  open,
  loading,
  error,
  detail,
  revealLine,
  findingMessage,
  onOpenChange,
}: TaskExecutionChangeDetailDialogProps) {
  const { t } = useTranslation("tasks");
  const editorContainerRef = useRef<HTMLDivElement | null>(null);
  const diffEditorRef = useRef<Monaco.editor.IStandaloneDiffEditor | null>(null);
  const standaloneEditorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const originalModelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const modifiedModelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const decorationsRef = useRef<Monaco.editor.IEditorDecorationsCollection | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const [editorError, setEditorError] = useState<string | null>(null);
  const [lineLocated, setLineLocated] = useState(false);
  const [revealChecked, setRevealChecked] = useState(false);

  const hasBeforeText = detail?.before_status === "text" && detail.before_text !== null;
  const hasAfterText = detail?.after_status === "text" && detail.after_text !== null;
  const hasDiffText = detail?.diff_text !== null && detail?.diff_text !== undefined;
  const useDiffEditor = Boolean(hasBeforeText || hasAfterText);
  const useUnifiedDiff = !useDiffEditor && Boolean(hasDiffText);
  const requestedRevealLine = revealLine && revealLine > 0 ? revealLine : null;
  const language = useMemo(
    () => detectMonacoLanguage(detail?.change.path ?? ""),
    [detail?.change.path],
  );

  useEffect(() => {
    if (
      !open ||
      loading ||
      error ||
      editorError ||
      !detail ||
      detail.snapshot_status !== "ready" ||
      (!useDiffEditor && !useUnifiedDiff)
    ) {
      return;
    }

    let cancelled = false;

    void loadMonaco()
      .then((monaco) => {
        if (cancelled || !editorContainerRef.current) {
          return;
        }

        decorationsRef.current?.clear();
        decorationsRef.current = null;
        originalModelRef.current?.dispose();
        modifiedModelRef.current?.dispose();
        originalModelRef.current = null;
        modifiedModelRef.current = null;

        if (useDiffEditor) {
          standaloneEditorRef.current?.dispose();
          standaloneEditorRef.current = null;
          originalModelRef.current = monaco.editor.createModel(detail.before_text ?? "", language);
          modifiedModelRef.current = monaco.editor.createModel(detail.after_text ?? "", language);
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

          const lineCount = modifiedModelRef.current.getLineCount();
          const canReveal =
            Boolean(requestedRevealLine) &&
            hasAfterText &&
            requestedRevealLine !== null &&
            requestedRevealLine <= lineCount;
          if (canReveal && requestedRevealLine) {
            const modifiedEditor = diffEditorRef.current.getModifiedEditor();
            const line = requestedRevealLine;
            const reveal = () => {
              if (cancelled || !diffEditorRef.current) {
                return;
              }
              diffEditorRef.current.layout();
              modifiedEditor.revealLineInCenter(line);
            };
            reveal();
            requestAnimationFrame(reveal);
            decorationsRef.current = modifiedEditor.createDecorationsCollection([
              {
                range: new monaco.Range(line, 1, line, 1),
                options: {
                  isWholeLine: true,
                  className: "review-finding-line",
                  linesDecorationsClassName: "review-finding-line-margin",
                },
              },
            ]);
          }
          setLineLocated(canReveal);
          setRevealChecked(true);
        } else {
          diffEditorRef.current?.dispose();
          diffEditorRef.current = null;
          modifiedModelRef.current = monaco.editor.createModel(detail.diff_text ?? "", "plaintext");
          if (!standaloneEditorRef.current) {
            standaloneEditorRef.current = monaco.editor.create(editorContainerRef.current, {
              model: modifiedModelRef.current,
              theme: getMonacoThemeName(),
              readOnly: true,
              automaticLayout: true,
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              wordWrap: "on",
              fontSize: 13,
            });
          } else {
            standaloneEditorRef.current.setModel(modifiedModelRef.current);
          }
          standaloneEditorRef.current.layout();
          setLineLocated(false);
          setRevealChecked(true);
        }

        resizeObserverRef.current?.disconnect();
        resizeObserverRef.current = new ResizeObserver(() => {
          diffEditorRef.current?.layout();
          standaloneEditorRef.current?.layout();
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
  }, [
    detail,
    editorError,
    error,
    hasAfterText,
    language,
    loading,
    open,
    requestedRevealLine,
    useDiffEditor,
    useUnifiedDiff,
  ]);

  useEffect(() => {
    if (!open) {
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
      decorationsRef.current?.clear();
      decorationsRef.current = null;
      diffEditorRef.current?.dispose();
      diffEditorRef.current = null;
      standaloneEditorRef.current?.dispose();
      standaloneEditorRef.current = null;
      originalModelRef.current?.dispose();
      originalModelRef.current = null;
      modifiedModelRef.current?.dispose();
      modifiedModelRef.current = null;
      setEditorError(null);
      setLineLocated(false);
      setRevealChecked(false);
    }
  }, [open]);

  const showCannotLocate =
    (Boolean(requestedRevealLine) || Boolean(findingMessage)) &&
    revealChecked &&
    !loading &&
    !error &&
    Boolean(detail) &&
    !lineLocated;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]">
        <DialogHeader>
          <DialogTitle>
            {detail
              ? `${getExecutionChangeTypeLabel(detail.change.change_type)} ${detail.change.path}`
              : t("detail.changeDetail.titleFallback")}
          </DialogTitle>
          <DialogDescription>{t("detail.changeDetail.description")}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            {t("detail.changeDetail.loading")}
          </div>
        ) : error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive">
            {error}
          </div>
        ) : !detail ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            {t("detail.changeDetail.empty")}
          </div>
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={`rounded-md border px-2 py-1 text-xs font-medium ${getExecutionChangeTypeClassName(detail.change.change_type)}`}
              >
                {getExecutionChangeTypeLabel(detail.change.change_type)}
              </span>
              <span className="rounded-md border border-border/70 bg-muted/30 px-2 py-1 text-xs text-muted-foreground">
                {getExecutionChangeCaptureModeLabel(detail.change.capture_mode)}
              </span>
            </div>

            <div className="rounded-md border border-border/70 bg-muted/20 p-3 text-xs text-muted-foreground">
              {getExecutionChangeCaptureModeDescription(detail.change.capture_mode)}
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">
                  {t("detail.changeDetail.absolutePath")}
                </div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.absolute_path ?? t("detail.labels.notRecorded")}
                </div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">
                  {t("detail.changeDetail.workingDir")}
                </div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.working_dir ?? t("detail.labels.notRecorded")}
                </div>
              </div>
              {detail.change.previous_path && (
                <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs md:col-span-2">
                  <div className="font-medium text-foreground">
                    {t("detail.changeDetail.previousPath")}
                  </div>
                  <div className="mt-1 whitespace-pre-wrap break-all font-mono text-muted-foreground">
                    {detail.change.previous_path}
                    {detail.previous_absolute_path ? `\n${detail.previous_absolute_path}` : ""}
                  </div>
                </div>
              )}
            </div>

            {detail.snapshot_message && (
              <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
                {detail.snapshot_message}
              </div>
            )}

            {findingMessage && (
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs text-foreground">
                {findingMessage}
              </div>
            )}

            {showCannotLocate && (
              <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                {useUnifiedDiff
                  ? t("detail.changeDetail.unifiedDiffOnly")
                  : t("detail.changeDetail.cannotLocateLine")}
              </div>
            )}

            {editorError && (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive">
                {editorError}
              </div>
            )}

            {detail.snapshot_status === "ready" && (
              <>
                <div className="grid gap-2 md:grid-cols-2">
                  <SnapshotMeta
                    label={t("detail.changeDetail.beforeSnapshot")}
                    status={detail.before_status}
                    truncated={detail.before_truncated}
                  />
                  <SnapshotMeta
                    label={t("detail.changeDetail.afterSnapshot")}
                    status={detail.after_status}
                    truncated={detail.after_truncated}
                  />
                </div>

                {detail.diff_truncated && (
                  <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                    {t("detail.changeDetail.diffTruncated")}
                  </div>
                )}

                {useDiffEditor || useUnifiedDiff ? (
                  <div
                    ref={editorContainerRef}
                    className="h-[36rem] overflow-hidden rounded-md border border-border/70 bg-background"
                  />
                ) : (
                  <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                    {t("detail.changeDetail.noTextContent")}
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
