import { useEffect, useState } from "react";
import {
  CODEX_MODEL_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  normalizeCodexModel,
  normalizeReasoningEffort,
  type CodexModelId,
  type Employee,
  type ReasoningEffort,
} from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const EMPLOYEE_ROLE_OPTIONS = [
  { value: "developer", label: "开发者" },
  { value: "reviewer", label: "审查员" },
  { value: "tester", label: "测试员" },
  { value: "coordinator", label: "协调员" },
] as const;

const NO_PROJECT_VALUE = "__no_project__";

interface EditEmployeeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  employee: Employee | null;
}

export function EditEmployeeDialog({ open, onOpenChange, employee }: EditEmployeeDialogProps) {
  const { updateEmployee } = useEmployeeStore();
  const { projects, fetchProjects } = useProjectStore();
  const [name, setName] = useState("");
  const [role, setRole] = useState("developer");
  const [model, setModel] = useState<CodexModelId>("gpt-5.4");
  const [reasoningEffort, setReasoningEffort] = useState<ReasoningEffort>("high");
  const [specialization, setSpecialization] = useState("");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [projectId, setProjectId] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open || !employee) return;

    fetchProjects();
    setName(employee.name);
    setRole(employee.role);
    setModel(normalizeCodexModel(employee.model));
    setReasoningEffort(normalizeReasoningEffort(employee.reasoning_effort));
    setSpecialization(employee.specialization ?? "");
    setSystemPrompt(employee.system_prompt ?? "");
    setProjectId(employee.project_id ?? "");
  }, [employee, fetchProjects, open]);

  const handleSave = async () => {
    if (!employee || !name.trim()) return;

    setSaving(true);
    try {
      await updateEmployee(employee.id, {
        name: name.trim(),
        role,
        model,
        reasoning_effort: reasoningEffort,
        specialization: specialization.trim() || null,
        system_prompt: systemPrompt.trim() || null,
        project_id: projectId || null,
      });
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>编辑员工</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">名称 *</label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="员工名称"
              className="mt-1"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">角色</label>
              <Select
                value={role}
                onValueChange={(value) => {
                  if (value) {
                    setRole(value);
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? EMPLOYEE_ROLE_OPTIONS.find((option) => option.value === value)?.label ?? value
                        : "选择角色"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {EMPLOYEE_ROLE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div>
              <label className="text-xs font-medium text-muted-foreground">模型</label>
              <Select
                value={model}
                onValueChange={(value) => {
                  if (value) {
                    setModel(value as CodexModelId);
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? CODEX_MODEL_OPTIONS.find((option) => option.value === value)?.label ?? value
                        : "选择模型"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {CODEX_MODEL_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">推理强度</label>
            <Select
              value={reasoningEffort}
              onValueChange={(value) => {
                if (value) {
                  setReasoningEffort(value as ReasoningEffort);
                }
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string"
                      ? REASONING_EFFORT_OPTIONS.find((option) => option.value === value)?.label ?? value
                      : "选择推理强度"
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {REASONING_EFFORT_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">专长</label>
            <Input
              value={specialization}
              onChange={(e) => setSpecialization(e.target.value)}
              placeholder="例如：全栈开发、代码审查"
              className="mt-1"
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">系统提示词</label>
            <textarea
              value={systemPrompt}
              onChange={(e) => setSystemPrompt(e.target.value)}
              placeholder="AI 员工的系统提示词（可选）"
              className="w-full mt-1 text-sm border border-input rounded-md p-2 bg-background min-h-[60px] resize-y"
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">关联项目</label>
            <Select
              value={projectId || NO_PROJECT_VALUE}
              onValueChange={(value) => {
                setProjectId(!value || value === NO_PROJECT_VALUE ? "" : value);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => {
                    if (!value || value === NO_PROJECT_VALUE) {
                      return "无";
                    }

                    return projects.find((project) => project.id === value)?.name ?? "无";
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_PROJECT_VALUE}>无</SelectItem>
                {projects.map((project) => (
                  <SelectItem key={project.id} value={project.id}>
                    {project.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={() => onOpenChange(false)}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent"
            >
              取消
            </button>
            <button
              onClick={handleSave}
              disabled={!name.trim() || !employee || saving}
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? "保存中..." : "保存"}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
