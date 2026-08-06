import { useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Eraser } from "lucide-react";

import { getLineColor } from "@/components/tasks/detail/taskDetailViewHelpers";
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
}

interface TerminalLine {
  key: string;
  line: string;
}

export function CodexTerminal({
  taskId,
  sessionRecordId,
  sessionKind = "execution",
  lines,
  hideClear = false,
  className = "",
  heightClassName = "h-36",
}: CodexTerminalProps) {
  const clearTaskCodexOutput = useEmployeeStore((s) => s.clearTaskCodexOutput);
  const clearSessionCodexOutput = useEmployeeStore((s) => s.clearSessionCodexOutput);
  const taskLogs = useEmployeeStore((s) => s.taskLogs);
  const sessionLogs = useEmployeeStore((s) => s.sessionLogs);

  const output: TerminalLine[] = useMemo(() => {
    if (lines) {
      return lines.map((line, index) => ({
        key: `filter:${index}:${line.slice(0, 48)}`,
        line,
      }));
    }
    if (sessionRecordId) {
      return (sessionLogs[sessionRecordId] ?? []).map((entry) => ({
        key: entry.event_id,
        line: entry.line,
      }));
    }
    if (taskId) {
      return (taskLogs[buildTaskLogKey(taskId, sessionKind)] ?? []).map((line, index) => ({
        key: `${index}:${line}`,
        line,
      }));
    }
    return [];
  }, [lines, sessionLogs, sessionRecordId, taskId, taskLogs, sessionKind]);

  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const shouldVirtualize = output.length >= VIRTUALIZE_THRESHOLD;

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
            title="清空日志"
          >
            <Eraser className="h-3 w-3" />
          </button>
        )}
      </div>
      {shouldVirtualize ? (
        <div
          ref={scrollParentRef}
          className={`${heightClassName} overflow-y-auto bg-black rounded-b p-2 font-mono text-xs`}
        >
          {output.length === 0 ? (
            <div className="text-zinc-600">暂无输出</div>
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
                    className={`absolute left-0 top-0 w-full whitespace-pre-wrap ${getLineColor(item.line)}`}
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
        <ScrollArea className={`${heightClassName} bg-black rounded-b`}>
          <div className="p-2 font-mono text-xs space-y-0.5">
            {output.length === 0 ? (
              <div className="text-zinc-600">暂无输出</div>
            ) : (
              output.map((item) => (
                <div key={item.key} className={`whitespace-pre-wrap ${getLineColor(item.line)}`}>
                  {item.line}
                </div>
              ))
            )}
            <div ref={bottomRef} />
          </div>
        </ScrollArea>
      )}
    </div>
  );
}
