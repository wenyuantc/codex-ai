import { useState, useEffect } from "react";
import { suggestAssignee } from "@/lib/ai";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Bot, Loader2, Check } from "lucide-react";

interface AISuggestDialogProps {
  taskId: string;
  taskDescription: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function AISuggestDialog({
  taskId,
  taskDescription,
  open,
  onOpenChange,
}: AISuggestDialogProps) {
  const { updateTask } = useTaskStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const [suggestion, setSuggestion] = useState<{ employeeId: string; reason: string } | null>(null);
  const [loading, setLoading] = useState(false);
  const [applied, setApplied] = useState(false);

  useEffect(() => {
    if (open) {
      fetchEmployees();
      setSuggestion(null);
      setApplied(false);
      setLoading(true);
      suggestAssignee(taskDescription).then((result) => {
        setSuggestion(result);
        setLoading(false);
      });
    }
  }, [open, taskDescription, fetchEmployees]);

  const handleApply = async () => {
    if (suggestion?.employeeId) {
      await updateTask(taskId, { assignee_id: suggestion.employeeId });
      setApplied(true);
      setTimeout(() => onOpenChange(false), 800);
    }
  };

  const suggestedEmployee = suggestion
    ? employees.find((e) => e.id === suggestion.employeeId)
    : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bot className="h-4 w-4 text-primary" />
            AI 指派建议
          </DialogTitle>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center gap-2 py-4 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            正在分析任务并推荐最佳员工...
          </div>
        ) : suggestion ? (
          <div className="space-y-4">
            {suggestedEmployee ? (
              <div className="bg-card rounded-lg border border-border p-4">
                <div className="flex items-center gap-3">
                  <div className="h-10 w-10 rounded-full bg-primary/10 flex items-center justify-center text-primary font-semibold">
                    {suggestedEmployee.name[0]}
                  </div>
                  <div>
                    <div className="font-medium text-sm">{suggestedEmployee.name}</div>
                    <div className="text-xs text-muted-foreground">
                      {suggestedEmployee.role} ·{" "}
                      {suggestedEmployee.specialization ?? suggestedEmployee.model}
                    </div>
                  </div>
                </div>
                <p className="text-xs text-muted-foreground mt-2">推荐理由：{suggestion.reason}</p>
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">{suggestion.reason}</p>
            )}

            <div className="flex justify-end gap-2">
              <button
                onClick={() => onOpenChange(false)}
                className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent"
              >
                取消
              </button>
              <button
                onClick={handleApply}
                disabled={!suggestion.employeeId || applied}
                className="flex items-center gap-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
              >
                {applied ? (
                  <>
                    <Check className="h-3 w-3" />
                    已应用
                  </>
                ) : (
                  "应用建议"
                )}
              </button>
            </div>
          </div>
        ) : (
          <p className="text-sm text-muted-foreground py-4">无法生成建议</p>
        )}
      </DialogContent>
    </Dialog>
  );
}
