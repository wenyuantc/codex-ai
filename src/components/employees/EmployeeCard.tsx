import { useEffect, useState } from "react";
import { CODEX_MODEL_OPTIONS, REASONING_EFFORT_OPTIONS, type Employee } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { EmployeeStatusBadge } from "./EmployeeStatusBadge";
import { DeleteEmployeeDialog } from "./DeleteEmployeeDialog";
import { EditEmployeeDialog } from "./EditEmployeeDialog";
import { CodexControls } from "@/components/codex/CodexControls";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Progress } from "@/components/ui/progress";
import { Trash2, Terminal, ChevronDown, Pencil } from "lucide-react";

interface EmployeeCardProps {
  employee: Employee;
  taskCount?: number;
}

const MAX_TASKS = 5;

export function EmployeeCard({ employee, taskCount = 0 }: EmployeeCardProps) {
  const deleteEmployee = useEmployeeStore((s) => s.deleteEmployee);
  const updateEmployeeStatus = useEmployeeStore((s) => s.updateEmployeeStatus);
  const clearCodexOutput = useEmployeeStore((s) => s.clearCodexOutput);
  const isRunning = useEmployeeStore((s) => s.codexProcesses[employee.id]?.running ?? false);
  const [showTerminal, setShowTerminal] = useState(false);
  const [showEdit, setShowEdit] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const workload = Math.min((taskCount / MAX_TASKS) * 100, 100);

  useEffect(() => {
    if (isRunning) {
      setShowTerminal(true);
    }
  }, [isRunning]);

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await updateEmployeeStatus(employee.id, "offline");
      clearCodexOutput(employee.id);
      await deleteEmployee(employee.id);
      setShowDeleteDialog(false);
    } finally {
      setDeleting(false);
    }
  };

  const roleLabels: Record<string, string> = {
    developer: "开发者",
    reviewer: "审查员",
    tester: "测试员",
    coordinator: "协调员",
  };
  const modelLabel = CODEX_MODEL_OPTIONS.find((option) => option.value === employee.model)?.label ?? employee.model;
  const reasoningLabel = REASONING_EFFORT_OPTIONS.find((option) => option.value === employee.reasoning_effort)?.label ?? employee.reasoning_effort;

  return (
    <div className="bg-card rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="p-4">
        <div className="flex items-center gap-3">
          <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center text-primary font-semibold shrink-0">
            {employee.name[0]}
          </div>
          <div className="flex-1 min-w-0">
            <div className="font-medium text-sm truncate">{employee.name}</div>
            <div className="text-xs text-muted-foreground">
              {roleLabels[employee.role] ?? employee.role}
              {employee.specialization && ` · ${employee.specialization}`}
            </div>
            <div className="text-[11px] text-muted-foreground/80 truncate">
              {modelLabel} · 推理{reasoningLabel}
            </div>
          </div>
          <EmployeeStatusBadge status={employee.status} />
        </div>

        {/* Workload */}
        {taskCount > 0 && (
          <div className="mt-3">
            <div className="flex justify-between text-xs text-muted-foreground mb-1">
              <span>工作负载</span>
              <span>{taskCount}/{MAX_TASKS}</span>
            </div>
            <Progress value={workload} className="h-1.5" />
          </div>
        )}
      </div>

      {/* Controls */}
      <div className="px-4 pb-3">
        <CodexControls
          employeeId={employee.id}
          employeeRole={employee.role}
          employeeStatus={employee.status}
          model={employee.model}
          reasoningEffort={employee.reasoning_effort}
          systemPrompt={employee.system_prompt}
        />
      </div>

      {/* Terminal toggle */}
      <Collapsible open={showTerminal} onOpenChange={setShowTerminal}>
        <CollapsibleTrigger className="w-full flex items-center justify-center gap-1.5 px-4 py-1.5 text-xs text-muted-foreground hover:bg-accent/50 transition-colors border-t border-border">
          <Terminal className="h-3 w-3" />
          {showTerminal ? "收起日志" : "查看日志"}
          <ChevronDown className={`h-3 w-3 transition-transform ${showTerminal ? "rotate-180" : ""}`} />
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="px-3 pb-3">
            <CodexTerminal employeeId={employee.id} />
          </div>
        </CollapsibleContent>
      </Collapsible>

      {/* Actions */}
      <div className="px-4 pb-3 flex justify-end gap-2">
        <button
          onClick={() => setShowEdit(true)}
          className="p-1 text-muted-foreground transition-colors hover:text-foreground"
          title="编辑员工"
        >
          <Pencil className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={() => setShowDeleteDialog(true)}
          disabled={deleting}
          className="p-1 text-muted-foreground transition-colors hover:text-destructive disabled:opacity-50"
          title="删除员工"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </button>
      </div>

      <EditEmployeeDialog
        open={showEdit}
        onOpenChange={setShowEdit}
        employee={employee}
      />

      <DeleteEmployeeDialog
        open={showDeleteDialog}
        onOpenChange={(open) => {
          if (!deleting) setShowDeleteDialog(open);
        }}
        employee={employee}
        deleting={deleting}
        onConfirm={handleDelete}
      />
    </div>
  );
}
