import { useEffect, useRef } from "react";
import type { CodexSessionKind } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { buildTaskLogKey } from "@/stores/employeeStore";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Eraser } from "lucide-react";

interface CodexTerminalProps {
  employeeId?: string;
  taskId?: string;
  sessionRecordId?: string;
  sessionKind?: CodexSessionKind;
}

export function CodexTerminal({ employeeId, taskId, sessionRecordId, sessionKind = "execution" }: CodexTerminalProps) {
  const codexProcesses = useEmployeeStore((s) => s.codexProcesses);
  const clearCodexOutput = useEmployeeStore((s) => s.clearCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((s) => s.clearTaskCodexOutput);
  const clearSessionCodexOutput = useEmployeeStore((s) => s.clearSessionCodexOutput);
  const taskLogs = useEmployeeStore((s) => s.taskLogs);
  const sessionLogs = useEmployeeStore((s) => s.sessionLogs);
  const process = employeeId ? codexProcesses[employeeId] : undefined;
  const output = sessionRecordId
    ? (sessionLogs[sessionRecordId] ?? []).map((entry) => ({
        key: entry.event_id,
        line: entry.line,
      }))
    : (taskId
      ? taskLogs[buildTaskLogKey(taskId, sessionKind)] ?? []
      : process?.output ?? []
    ).map((line, index) => ({
      key: `${index}:${line}`,
      line,
    }));
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [output.length]);

  function getLineColor(line: string): string {
    if (line.startsWith("[ERROR]")) return "text-red-400";
    if (line.startsWith("[EXIT]")) return "text-yellow-400";
    return "text-green-400";
  }

  return (
    <div className="relative">
      <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
        <span className="text-xs text-zinc-500 font-mono">终端输出</span>
        <button
          onClick={() => {
            if (sessionRecordId) {
              clearSessionCodexOutput(sessionRecordId);
              return;
            }

            if (taskId) {
              clearTaskCodexOutput(taskId, sessionKind);
              return;
            }

            if (employeeId) {
              clearCodexOutput(employeeId);
            }
          }}
          className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
          title="清空日志"
        >
          <Eraser className="h-3 w-3" />
        </button>
      </div>
      <ScrollArea className="h-36 bg-black rounded-b">
        <div className="p-2 font-mono text-xs space-y-0.5">
          {output.length === 0 ? (
            <div className="text-zinc-600">暂无输出</div>
          ) : (
            output.map((line, i) => (
              <div key={output[i].key} className={`whitespace-pre-wrap ${getLineColor(line.line)}`}>
                {line.line}
              </div>
            ))
          )}
          <div ref={bottomRef} />
        </div>
      </ScrollArea>
    </div>
  );
}
