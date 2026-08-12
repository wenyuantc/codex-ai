import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Check, Copy, Download, Eraser } from "lucide-react";

import { formatTerminalLine, getLineColor } from "@/components/tasks/detail/taskDetailViewHelpers";
import { SessionInputBar } from "@/components/sessions/SessionInputBar";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { CodexSessionKind } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";

/** Lines at or above this count use virtualization (aligns with long session UX). */
const VIRTUALIZE_THRESHOLD = 80;
/** Conservative estimate for mono text-xs lines with wrapping variance absorbed by overscan. */
const ESTIMATED_LINE_HEIGHT = 20;
const VIRTUAL_OVERSCAN = 12;

interface CodexTerminalProps {
  taskId?: string;
  sessionRecordId?: string;
  sessionKind?: CodexSessionKind;
  /** Optional pre-filtered lines (e.g. keyword filter in SessionLogDialog). */
  lines?: string[];
  /** Hide clear button when showing external filtered lines. */
  hideClear?: boolean;
  className?: string;
  /** Override scroll area height class (default h-36). */
  heightClassName?: string;
  /** Mid-session input bar (TaskLog / SessionLog / EmployeeRunningSessions). */
  inputEmployeeId?: string | null;
  inputProvider?: string | null;
  inputSessionLive?: boolean;
  showInputBar?: boolean;
}

interface TerminalLine {
  key: string;
  /** Localized display text */
  line: string;
  /** Raw backend line (stable tags) for color rules */
  sourceLine: string;
}

export function CodexTerminal({
  taskId,
  sessionRecordId,
  sessionKind = "execution",
  lines,
  hideClear = false,
  className = "",
  heightClassName = "h-36",
  inputEmployeeId = null,
  inputProvider = null,
  inputSessionLive = true,
  showInputBar = false,
}: CodexTerminalProps) {
  const { t, i18n } = useTranslation("sessions");
  const clearTaskCodexOutput = useEmployeeStore((s) => s.clearTaskCodexOutput);
  const clearSessionCodexOutput = useEmployeeStore((s) => s.clearSessionCodexOutput);
  const taskLogs = useEmployeeStore((s) => s.taskLogs);
  const sessionLogs = useEmployeeStore((s) => s.sessionLogs);

  const output: TerminalLine[] = useMemo(() => {
    const toItem = (key: string, sourceLine: string): TerminalLine => ({
      key,
      sourceLine,
      line: formatTerminalLine(sourceLine),
    });
    if (lines) {
      return lines.map((line, index) => toItem(`filter:${index}:${line.slice(0, 48)}`, line));
    }
    if (sessionRecordId) {
      return (sessionLogs[sessionRecordId] ?? []).map((entry) =>
        toItem(entry.event_id, entry.line),
      );
    }
    if (taskId) {
      return (taskLogs[buildTaskLogKey(taskId, sessionKind)] ?? []).map((line, index) =>
        toItem(`${index}:${line}`, line),
      );
    }
    return [];
  }, [lines, sessionLogs, sessionRecordId, taskId, taskLogs, sessionKind, i18n.language]);

  const [logCopied, setLogCopied] = useState(false);
  const copyResetRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copyResetRef.current !== null) {
        window.clearTimeout(copyResetRef.current);
      }
    };
  }, []);

  const handleCopyLog = async () => {
    if (output.length === 0) {
      return;
    }
    try {
      await navigator.clipboard.writeText(output.map((item) => item.line).join("\n"));
      setLogCopied(true);
      if (copyResetRef.current !== null) {
        window.clearTimeout(copyResetRef.current);
      }
      copyResetRef.current = window.setTimeout(() => setLogCopied(false), 1500);
    } catch (error) {
      console.error("Failed to copy terminal log:", error);
    }
  };

  const handleExportLog = () => {
    if (output.length === 0) {
      return;
    }
    const content = output.map((item) => item.line).join("\n");
    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    const scope = sessionRecordId ?? taskId ?? "output";
    const timestamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
    anchor.href = url;
    anchor.download = `session-log-${scope}-${timestamp}.log`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const shouldVirtualize = output.length >= VIRTUALIZE_THRESHOLD;
  const bodyRoundedClass = showInputBar ? "rounded-none" : "rounded-b";

  const virtualizer = useVirtualizer({
    count: shouldVirtualize ? output.length : 0,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => ESTIMATED_LINE_HEIGHT,
    overscan: VIRTUAL_OVERSCAN,
    enabled: shouldVirtualize,
  });

  // Stick to bottom when line count changes only — do not depend on `virtualizer`
  // identity (it can change each render and fight user upward scroll).
  useEffect(() => {
    if (shouldVirtualize) {
      if (output.length > 0) {
        virtualizer.scrollToIndex(output.length - 1, { align: "end" });
      }
      return;
    }
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional: only re-stick on length / mode
  }, [output.length, shouldVirtualize]);

  return (
    <div className={`relative ${className}`}>
      <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
        <span className="text-xs text-zinc-500 font-mono">终端输出</span>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => void handleCopyLog()}
            disabled={output.length === 0}
            className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors disabled:cursor-not-allowed disabled:opacity-40"
            title={logCopied ? t("terminalCopied") : t("terminalCopy")}
          >
            {logCopied ? (
              <Check className="h-3 w-3 text-green-500" />
            ) : (
              <Copy className="h-3 w-3" />
            )}
          </button>
          <button
            type="button"
            onClick={handleExportLog}
            disabled={output.length === 0}
            className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors disabled:cursor-not-allowed disabled:opacity-40"
            title={t("terminalExport")}
          >
            <Download className="h-3 w-3" />
          </button>
          {!hideClear && (
            <button
              type="button"
              onClick={() => {
                if (sessionRecordId) {
                  clearSessionCodexOutput(sessionRecordId);
                  return;
                }
                if (taskId) {
                  clearTaskCodexOutput(taskId, sessionKind);
                }
              }}
              className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
              title={t("terminalClear")}
            >
              <Eraser className="h-3 w-3" />
            </button>
          )}
        </div>
      </div>
      {shouldVirtualize ? (
        <div
          ref={scrollParentRef}
          className={`${heightClassName} overflow-y-auto bg-black ${bodyRoundedClass} p-2 font-mono text-xs`}
        >
          {output.length === 0 ? (
            <div className="text-zinc-600">{t("terminalEmpty")}</div>
          ) : (
            <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const item = output[virtualRow.index];
                if (!item) {
                  return null;
                }
                return (
                  <div
                    key={item.key}
                    data-index={virtualRow.index}
                    ref={virtualizer.measureElement}
                    className={`absolute left-0 top-0 w-full whitespace-pre-wrap ${getLineColor(item.sourceLine)}`}
                    style={{ transform: `translateY(${virtualRow.start}px)` }}
                  >
                    {item.line}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      ) : (
        <ScrollArea className={`${heightClassName} bg-black ${bodyRoundedClass}`}>
          <div className="p-2 font-mono text-xs space-y-0.5">
            {output.length === 0 ? (
              <div className="text-zinc-600">{t("terminalEmpty")}</div>
            ) : (
              output.map((item) => (
                <div
                  key={item.key}
                  className={`whitespace-pre-wrap ${getLineColor(item.sourceLine)}`}
                >
                  {item.line}
                </div>
              ))
            )}
            <div ref={bottomRef} />
          </div>
        </ScrollArea>
      )}
      {showInputBar ? (
        <SessionInputBar
          employeeId={inputEmployeeId}
          provider={inputProvider}
          sessionLive={inputSessionLive}
        />
      ) : null}
    </div>
  );
}
