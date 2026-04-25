import { useEffect, useState } from "react";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  AI_PROVIDER_OPTIONS,
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  CLAUDE_THINKING_BUDGET_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  type AiProvider,
  getDefaultModelForProvider,
  getDefaultReasoningEffortForProvider,
  normalizeReasoningEffortForProvider,
} from "@/lib/types";
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
import { EmployeeSystemPromptField } from "./EmployeeSystemPromptField";

const EMPLOYEE_ROLE_OPTIONS = [
  { value: "developer", label: "开发者" },
  { value: "reviewer", label: "审查员" },
  { value: "tester", label: "测试员" },
  { value: "coordinator", label: "协调员" },
] as const;

const NO_PROJECT_VALUE = "__no_project__";

interface CreateEmployeeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultProjectId?: string;
}

export function CreateEmployeeDialog({
  open,
  onOpenChange,
  defaultProjectId,
}: CreateEmployeeDialogProps) {
  const { createEmployee } = useEmployeeStore();
  const { projects, fetchProjects } = useProjectStore();
  const [name, setName] = useState("");
  const [role, setRole] = useState("developer");
  const [aiProvider, setAiProvider] = useState<AiProvider>("codex");
  const [model, setModel] = useState<string>("gpt-5.4");
  const [reasoningEffort, setReasoningEffort] = useState<string>("high");
  const [specialization, setSpecialization] = useState("");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [projectId, setProjectId] = useState("");
  const [saving, setSaving] = useState(false);

  const modelOptions = aiProvider === "claude" ? CLAUDE_MODEL_OPTIONS : CODEX_MODEL_OPTIONS;
  const effortOptions = aiProvider === "claude" ? CLAUDE_THINKING_BUDGET_OPTIONS : REASONING_EFFORT_OPTIONS;

  const resetForm = () => {
    setName("");
    setRole("developer");
    setAiProvider("codex");
    setModel("gpt-5.4");
    setReasoningEffort("high");
    setSpecialization("");
    setSystemPrompt("");
    setProjectId(defaultProjectId ?? "");
  };

  useEffect(() => {
    if (open) {
      void fetchProjects();
      resetForm();
    }
  }, [defaultProjectId, fetchProjects, open]);

  const handleCreate = async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      await createEmployee({
        name: name.trim(),
        role,
        model,
        reasoning_effort: normalizeReasoningEffortForProvider(aiProvider, reasoningEffort),
        specialization: specialization.trim() || undefined,
        system_prompt: systemPrompt.trim() || undefined,
        project_id: projectId || undefined,
        ai_provider: aiProvider,
      });
      resetForm();
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>添加员工</DialogTitle>
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
              <label className="text-xs font-medium text-muted-foreground">AI 提供商</label>
              <Select
                value={aiProvider}
                onValueChange={(value) => {
                  if (value) {
                    const provider = value as AiProvider;
                    setAiProvider(provider);
                    setModel(getDefaultModelForProvider(provider) as string);
                    setReasoningEffort(getDefaultReasoningEffortForProvider(provider));
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? AI_PROVIDER_OPTIONS.find((option) => option.value === value)?.label ?? value
                        : "选择提供商"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {AI_PROVIDER_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">模型</label>
              <Select
                value={model}
                onValueChange={(value) => {
                  if (value) {
                    setModel(value);
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? modelOptions.find((option) => option.value === value)?.label ?? value
                        : "选择模型"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {modelOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div>
              <label className="text-xs font-medium text-muted-foreground">推理强度</label>
              <Select
                value={reasoningEffort}
                onValueChange={(value) => {
                  if (value) {
                    setReasoningEffort(value);
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? effortOptions.find((option) => option.value === value)?.label ?? value
                        : "选择推理强度"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {effortOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
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

          <EmployeeSystemPromptField
            open={open}
            role={role}
            specialization={specialization}
            systemPrompt={systemPrompt}
            projectId={projectId || undefined}
            disabled={saving}
            onSystemPromptChange={setSystemPrompt}
          />

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
              onClick={handleCreate}
              disabled={!name.trim() || saving}
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? "创建中..." : "创建"}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
