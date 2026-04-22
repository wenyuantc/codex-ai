import { useEffect } from "react";
import { Loader2, Sparkles } from "lucide-react";

import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { getProjectWorkingDir } from "@/lib/projects";
import { useProjectStore } from "@/stores/projectStore";
import { Button } from "@/components/ui/button";

const EMPLOYEE_PROMPT_FALLBACK_PROJECT_NAME = "通用 AI 员工";
const EMPLOYEE_PROMPT_FALLBACK_PROJECT_DESCRIPTION = "未关联项目，生成通用 AI 员工系统提示词。";
const EMPLOYEE_SYSTEM_PROMPT_MAX_LENGTH = 12000;

interface EmployeeSystemPromptFieldProps {
  open: boolean;
  role: string;
  specialization: string;
  systemPrompt: string;
  projectId?: string;
  disabled?: boolean;
  onSystemPromptChange: (value: string) => void;
}

export function EmployeeSystemPromptField({
  open,
  role,
  specialization,
  systemPrompt,
  projectId,
  disabled = false,
  onSystemPromptChange,
}: EmployeeSystemPromptFieldProps) {
  const projects = useProjectStore((state) => state.projects);
  const fetchProjects = useProjectStore((state) => state.fetchProjects);
  const optimizePrompt = useAiOptimizePrompt(open);
  const project = projectId
    ? projects.find((item) => item.id === projectId)
    : undefined;

  useEffect(() => {
    if (open) {
      void fetchProjects();
    }
  }, [fetchProjects, open]);

  useEffect(() => {
    if (open) {
      optimizePrompt.reset();
    }
  }, [open, projectId, role, specialization, systemPrompt]);

  const handleGeneratePrompt = async () => {
    let currentProject = project;

    if (projectId && !currentProject) {
      await fetchProjects();
      currentProject = useProjectStore.getState().projects.find((item) => item.id === projectId);
    }

    await optimizePrompt.generate({
      scene: "employee_system_prompt",
      projectId: currentProject?.id ?? null,
      projectName: currentProject?.name ?? EMPLOYEE_PROMPT_FALLBACK_PROJECT_NAME,
      projectDescription: currentProject?.description ?? EMPLOYEE_PROMPT_FALLBACK_PROJECT_DESCRIPTION,
      projectRepoPath: getProjectWorkingDir(currentProject),
      title: null,
      description: null,
      currentPrompt: null,
      taskTitle: null,
      sessionSummary: null,
      taskId: null,
      workingDir: getProjectWorkingDir(currentProject),
      employeeRole: role,
      employeeSpecialization: specialization,
      employeeDraftSystemPrompt: systemPrompt,
    });
  };

  const handleApplyOptimizedPrompt = () => {
    if (!optimizePrompt.optimizedPrompt) {
      return;
    }

    onSystemPromptChange(optimizePrompt.optimizedPrompt);
    optimizePrompt.reset();
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <label className="text-xs font-medium text-muted-foreground">系统提示词</label>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void handleGeneratePrompt()}
          disabled={disabled || optimizePrompt.loading}
        >
          {optimizePrompt.loading ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Sparkles className="h-3.5 w-3.5" />
          )}
          AI生成系统提示词
        </Button>
      </div>

      <textarea
        value={systemPrompt}
        onChange={(event) => onSystemPromptChange(event.target.value)}
        placeholder="AI 员工的系统提示词（可选）"
        className="w-full text-sm border border-input rounded-md p-2 bg-background min-h-[120px] resize-y"
        disabled={disabled}
        maxLength={EMPLOYEE_SYSTEM_PROMPT_MAX_LENGTH}
      />
      <div className="text-[11px] text-muted-foreground text-right">
        {systemPrompt.length}/{EMPLOYEE_SYSTEM_PROMPT_MAX_LENGTH}
      </div>

      {optimizePrompt.error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {optimizePrompt.error}
        </div>
      )}

      {optimizePrompt.optimizedPrompt && (
        <div className="space-y-3 rounded-md border border-primary/20 bg-primary/5 p-3">
          <div className="flex items-center justify-between gap-2">
            <div>
              <p className="text-xs font-medium text-primary">生成后的系统提示词</p>
              <p className="text-[11px] text-muted-foreground">确认后会替换当前系统提示词输入框内容</p>
            </div>
            <Button type="button" size="sm" onClick={handleApplyOptimizedPrompt}>
              替换提示词
            </Button>
          </div>
          <div className="max-h-56 overflow-y-auto rounded-md border bg-background/80 p-3 text-xs whitespace-pre-wrap text-foreground">
            {optimizePrompt.optimizedPrompt}
          </div>
        </div>
      )}
    </div>
  );
}
