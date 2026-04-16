import { useEffect, useState } from "react";
import { Loader2, Sparkles } from "lucide-react";

import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { useProjectStore } from "@/stores/projectStore";
import type { Task } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ContinueConversationDialogProps {
  open: boolean;
  task: Task | null;
  submitting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (prompt: string) => Promise<void> | void;
}

export function ContinueConversationDialog({
  open,
  task,
  submitting = false,
  onOpenChange,
  onConfirm,
}: ContinueConversationDialogProps) {
  const projects = useProjectStore((state) => state.projects);
  const fetchProjects = useProjectStore((state) => state.fetchProjects);
  const optimizePrompt = useAiOptimizePrompt(open);
  const [prompt, setPrompt] = useState("");
  const project = task?.project_id
    ? projects.find((item) => item.id === task.project_id)
    : undefined;

  useEffect(() => {
    if (open) {
      setPrompt("");
      void fetchProjects();
    }
  }, [fetchProjects, open, task?.id]);

  useEffect(() => {
    if (open) {
      optimizePrompt.reset();
    }
  }, [open, prompt, task?.id, task?.project_id]);

  const handleGenerateOptimizedPrompt = async () => {
    if (!task?.project_id) {
      optimizePrompt.showError("当前任务未关联有效项目，无法生成优化提示词。");
      return;
    }

    let currentProject = project;
    if (!currentProject) {
      await fetchProjects();
      currentProject = useProjectStore.getState().projects.find((item) => item.id === task.project_id);
    }

    if (!currentProject) {
      optimizePrompt.showError("当前任务未关联有效项目，无法生成优化提示词。");
      return;
    }

    await optimizePrompt.generate({
      scene: "task_continue",
      projectName: currentProject.name,
      projectDescription: currentProject.description,
      projectRepoPath: currentProject.repo_path,
      title: null,
      description: task.description,
      currentPrompt: prompt,
      taskTitle: task.title,
      sessionSummary: null,
      taskId: task.id,
      workingDir: currentProject.repo_path ?? null,
    });
  };

  const handleApplyOptimizedPrompt = () => {
    if (!optimizePrompt.optimizedPrompt) {
      return;
    }

    setPrompt(optimizePrompt.optimizedPrompt);
    optimizePrompt.reset();
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,42rem)] max-w-[min(96vw,42rem)] sm:max-w-[min(96vw,42rem)]">
        <DialogHeader>
          <DialogTitle>继续对话</DialogTitle>
          <DialogDescription>
            向任务“{task?.title ?? ""}”上一次 Codex session 发送新的续聊内容。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <div className="flex items-center justify-between gap-3">
            <label className="text-sm font-medium text-foreground" htmlFor="continue-conversation-prompt">
              对话内容
            </label>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void handleGenerateOptimizedPrompt()}
              disabled={submitting || optimizePrompt.loading}
            >
              {optimizePrompt.loading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Sparkles className="h-3.5 w-3.5" />
              )}
              AI优化提示词
            </Button>
          </div>
          <textarea
            id="continue-conversation-prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="输入想继续追问或继续执行的内容..."
            className="min-h-32 w-full resize-y rounded-md border border-input bg-background p-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
            disabled={submitting}
          />

          {optimizePrompt.error && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {optimizePrompt.error}
            </div>
          )}

          {optimizePrompt.optimizedPrompt && (
            <div className="space-y-3 rounded-md border border-primary/20 bg-primary/5 p-3">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <p className="text-xs font-medium text-primary">优化后的提示词</p>
                  <p className="text-[11px] text-muted-foreground">确认后会替换当前输入框内容</p>
                </div>
                <Button type="button" size="sm" onClick={handleApplyOptimizedPrompt}>
                  替换输入
                </Button>
              </div>
              <div className="max-h-56 overflow-y-auto rounded-md border bg-background/80 p-3 text-xs whitespace-pre-wrap text-foreground">
                {optimizePrompt.optimizedPrompt}
              </div>
            </div>
          )}
        </div>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            取消
          </Button>
          <Button
            type="button"
            onClick={() => void onConfirm(prompt.trim())}
            disabled={submitting || !prompt.trim()}
          >
            {submitting ? "执行中..." : "执行"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
