import { useEffect, useState } from "react";
import { Loader2, Sparkles } from "lucide-react";

import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { CodexSessionListItem } from "@/lib/types";
import { getProjectWorkingDir } from "@/lib/projects";
import { useProjectStore } from "@/stores/projectStore";

interface SessionContinueDialogProps {
  open: boolean;
  session: CodexSessionListItem | null;
  submitting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (prompt: string) => Promise<void> | void;
}

export function SessionContinueDialog({
  open,
  session,
  submitting = false,
  onOpenChange,
  onConfirm,
}: SessionContinueDialogProps) {
  const projects = useProjectStore((state) => state.projects);
  const fetchProjects = useProjectStore((state) => state.fetchProjects);
  const optimizePrompt = useAiOptimizePrompt(open);
  const [prompt, setPrompt] = useState("");
  const project = session?.project_id
    ? projects.find((item) => item.id === session.project_id)
    : undefined;

  useEffect(() => {
    if (open) {
      setPrompt("");
      void fetchProjects();
    }
  }, [fetchProjects, open, session?.session_record_id]);

  useEffect(() => {
    if (open) {
      optimizePrompt.reset();
    }
  }, [open, prompt, session?.session_record_id, session?.project_id]);

  const handleGenerateOptimizedPrompt = async () => {
    if (!session?.project_id) {
      optimizePrompt.showError("当前 Session 未关联项目，无法生成优化提示词。");
      return;
    }

    let currentProject = project;
    if (!currentProject) {
      await fetchProjects();
      currentProject = useProjectStore.getState().projects.find((item) => item.id === session.project_id);
    }

    if (!currentProject) {
      optimizePrompt.showError("当前 Session 未关联项目，无法生成优化提示词。");
      return;
    }

    await optimizePrompt.generate({
      scene: "session_continue",
      projectName: currentProject.name,
      projectDescription: currentProject.description,
      projectRepoPath: getProjectWorkingDir(currentProject),
      title: null,
      description: null,
      currentPrompt: prompt,
      taskTitle: session.task_title,
      sessionSummary: session.summary ?? session.content_preview ?? session.display_name,
      taskId: session.task_id,
      workingDir: session.working_dir ?? getProjectWorkingDir(currentProject),
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
            向 Session “{session?.display_name ?? "未命名会话"}” 发送新的续聊内容。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <div className="rounded-lg border border-border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
            <div className="font-mono">session id: {session?.session_id ?? "暂无"}</div>
            <div className="mt-1">关联任务：{session?.task_title ?? "无关联任务"}</div>
          </div>
          <div className="flex items-center justify-between gap-3">
            <label className="text-sm font-medium text-foreground" htmlFor="session-continue-prompt">
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
            id="session-continue-prompt"
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
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
            {submitting ? "执行中..." : "执行并查看日志"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
