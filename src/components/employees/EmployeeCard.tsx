import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  GROK_MODEL_OPTIONS,
  formatEmployeeAiProviderLabel,
  normalizeClaudeModel,
  normalizeGrokModel,
  type Employee,
} from "@/lib/types";
import { mapBackendError } from "@/lib/i18n/mapBackendError";
import { getReasoningEffortLabel } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { EmployeeStatusBadge } from "./EmployeeStatusBadge";
import { DeleteEmployeeDialog } from "./DeleteEmployeeDialog";
import { EditEmployeeDialog } from "./EditEmployeeDialog";
import { EmployeeRunningSessionsDialog } from "./EmployeeRunningSessionsDialog";
import { CodexControls } from "@/components/codex/CodexControls";
import { Progress } from "@/components/ui/progress";
import { Trash2, Terminal, Pencil } from "lucide-react";

interface EmployeeCardProps {
  employee: Employee;
  taskCount?: number;
  highlighted?: boolean;
}

const MAX_TASKS = 5;

export function EmployeeCard({ employee, taskCount = 0, highlighted = false }: EmployeeCardProps) {
  const { t } = useTranslation(["employees", "common"]);
  const deleteEmployee = useEmployeeStore((s) => s.deleteEmployee);
  const updateEmployeeStatus = useEmployeeStore((s) => s.updateEmployeeStatus);
  const employeeRuntime = useEmployeeStore((s) => s.employeeRuntime[employee.id]);
  const runningSessions = employeeRuntime?.sessions ?? [];
  const [showEdit, setShowEdit] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [showRunningDialog, setShowRunningDialog] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const workload = Math.min((taskCount / MAX_TASKS) * 100, 100);

  const handleDelete = async () => {
    setDeleting(true);
    setDeleteError(null);
    try {
      await updateEmployeeStatus(employee.id, "offline");
      await deleteEmployee(employee.id);
      setShowDeleteDialog(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setDeleteError(mapBackendError(message));
      console.error("Failed to delete employee:", error);
    } finally {
      setDeleting(false);
    }
  };

  const roleLabel = t(`common:role.${employee.role}`, { defaultValue: employee.role });
  const allModelOptions =
    employee.ai_provider === "claude"
      ? CLAUDE_MODEL_OPTIONS
      : employee.ai_provider === "opencode" || employee.ai_provider === "native"
        ? []
        : employee.ai_provider === "grok"
          ? GROK_MODEL_OPTIONS
          : CODEX_MODEL_OPTIONS;
  const displayModel =
    employee.ai_provider === "claude"
      ? normalizeClaudeModel(employee.model)
      : employee.ai_provider === "grok"
        ? normalizeGrokModel(employee.model)
        : employee.model;
  const modelLabel =
    allModelOptions.find((option) => option.value === displayModel)?.label ?? displayModel;
  const reasoningLabel = getReasoningEffortLabel(employee.reasoning_effort, employee.ai_provider);
  const providerLabel = formatEmployeeAiProviderLabel(employee.ai_provider);

  return (
    <div
      id={`employee-card-${employee.id}`}
      className={`overflow-hidden rounded-lg border bg-card ${
        highlighted ? "border-primary ring-2 ring-primary/20" : "border-border"
      }`}
    >
      {/* Header */}
      <div className="p-4">
        <div className="flex items-center gap-3">
          <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center text-primary font-semibold shrink-0">
            {employee.name[0]}
          </div>
          <div className="flex-1 min-w-0">
            <div className="font-medium text-sm truncate">{employee.name}</div>
            <div className="text-xs text-muted-foreground">
              {roleLabel}
              {employee.specialization && ` · ${employee.specialization}`}
            </div>
            <div className="text-[11px] text-muted-foreground/80 truncate">
              <span
                className={
                  employee.ai_provider === "claude"
                    ? "text-orange-500"
                    : employee.ai_provider === "opencode"
                      ? "text-blue-500"
                      : employee.ai_provider === "grok"
                        ? "text-purple-500"
                        : employee.ai_provider === "native"
                          ? "text-cyan-500"
                          : "text-green-500"
                }
              >
                {providerLabel}
                {" · "}
                {modelLabel} · {t("modelReasoning", { effort: reasoningLabel })}
              </span>
            </div>
          </div>
          <EmployeeStatusBadge status={employee.status} />
        </div>

        {/* Workload */}
        {taskCount > 0 && (
          <div className="mt-3">
            <div className="flex justify-between text-xs text-muted-foreground mb-1">
              <span>{t("workload")}</span>
              <span>
                {taskCount}/{MAX_TASKS}
              </span>
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
          aiProvider={employee.ai_provider}
        />
      </div>

      {runningSessions.length > 0 && (
        <div className="px-4 pb-3">
          <button
            type="button"
            onClick={() => setShowRunningDialog(true)}
            className="w-full flex items-center justify-center gap-1.5 rounded-md border border-border px-3 py-2 text-xs text-muted-foreground hover:bg-accent/50 transition-colors"
          >
            <Terminal className="h-3 w-3" />
            {t("viewRunningTerminal")}
          </button>
        </div>
      )}

      {/* Actions */}
      <div className="flex justify-end gap-2 border-t border-border px-4 pb-3 pt-3">
        <button
          onClick={() => setShowEdit(true)}
          className="p-1 text-muted-foreground transition-colors hover:text-foreground"
          title={t("editEmployeeTitle")}
        >
          <Pencil className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={() => {
            setDeleteError(null);
            setShowDeleteDialog(true);
          }}
          disabled={deleting}
          className="p-1 text-muted-foreground transition-colors hover:text-destructive disabled:opacity-50"
          title={t("deleteEmployeeTitle")}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </button>
      </div>

      <EmployeeRunningSessionsDialog
        open={showRunningDialog}
        employee={employee}
        sessions={runningSessions}
        onOpenChange={setShowRunningDialog}
      />

      <EditEmployeeDialog open={showEdit} onOpenChange={setShowEdit} employee={employee} />

      <DeleteEmployeeDialog
        open={showDeleteDialog}
        onOpenChange={(open) => {
          if (deleting) {
            return;
          }
          if (!open) {
            setDeleteError(null);
          }
          setShowDeleteDialog(open);
        }}
        employee={employee}
        deleting={deleting}
        error={deleteError}
        onConfirm={handleDelete}
      />
    </div>
  );
}
