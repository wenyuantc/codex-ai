import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { ImagePlus, Loader2, Sparkles } from "lucide-react";

import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { IMAGE_FILE_FILTERS, dedupePaths, isTauriRuntime, normalizeDialogSelection } from "@/lib/taskAttachments";
import { PRIORITIES } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TaskAttachmentGrid } from "./TaskAttachmentGrid";

const UNASSIGNED_VALUE = "__unassigned__";

interface CreateTaskDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId?: string;
}

export function CreateTaskDialog({
  open,
  onOpenChange,
  projectId,
}: CreateTaskDialogProps) {
  const { createTask } = useTaskStore();
  const { projects, fetchProjects } = useProjectStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const optimizePrompt = useAiOptimizePrompt(open);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("medium");
  const [selectedProjectId, setSelectedProjectId] = useState(
    projectId ?? ""
  );
  const [assigneeId, setAssigneeId] = useState("");
  const [attachmentPaths, setAttachmentPaths] = useState<string[]>([]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const selectedProject = projects.find((project) => project.id === selectedProjectId);

  useEffect(() => {
    if (open) {
      optimizePrompt.reset();
    }
  }, [open, selectedProjectId, title, description]);

  const handleOpen = (isOpen: boolean) => {
    if (isOpen) {
      fetchEmployees();
      fetchProjects();
      setTitle("");
      setDescription("");
      setPriority("medium");
      setSelectedProjectId(projectId ?? "");
      setAssigneeId("");
      setAttachmentPaths([]);
      setCreateError(null);
    }
    onOpenChange(isOpen);
  };

  const handleSelectAttachments = async () => {
    const selected = await openFileDialog({
      directory: false,
      multiple: true,
      filters: IMAGE_FILE_FILTERS,
      title: "选择任务图片",
    });

    const nextPaths = dedupePaths([
      ...attachmentPaths,
      ...normalizeDialogSelection(selected),
    ]);
    setAttachmentPaths(nextPaths);
  };

  const handleGenerateOptimizedDescription = async () => {
    if (!selectedProjectId) {
      optimizePrompt.showError("请先选择项目后再生成优化提示词。");
      return;
    }

    if (!selectedProject) {
      optimizePrompt.showError("当前项目不存在，无法生成优化提示词。");
      return;
    }

    await optimizePrompt.generate({
      scene: "task_create",
      projectName: selectedProject.name,
      projectDescription: selectedProject.description,
      projectRepoPath: selectedProject.repo_path,
      title,
      description,
      currentPrompt: null,
      taskTitle: null,
      sessionSummary: null,
      taskId: null,
      workingDir: selectedProject.repo_path ?? null,
    });
  };

  const handleApplyOptimizedDescription = () => {
    if (!optimizePrompt.optimizedPrompt) {
      return;
    }

    setDescription(optimizePrompt.optimizedPrompt);
    optimizePrompt.reset();
  };

  const handleCreate = async () => {
    if (!title.trim() || !selectedProjectId) return;
    setCreateError(null);
    setSaving(true);
    try {
      await createTask({
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        project_id: selectedProjectId,
        assignee_id: assigneeId || undefined,
        attachment_source_paths: attachmentPaths,
      }, {
        refreshProjectId: projectId,
      });
      handleOpen(false);
    } catch (error) {
      setCreateError(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpen}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>新建任务</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              标题 *
            </label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="任务标题"
              className="mt-1"
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <label className="text-xs font-medium text-muted-foreground">
                描述
              </label>
              <button
                type="button"
                onClick={() => void handleGenerateOptimizedDescription()}
                disabled={saving || optimizePrompt.loading}
                className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
              >
                {optimizePrompt.loading ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Sparkles className="h-3.5 w-3.5" />
                )}
                AI优化提示词
              </button>
            </div>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="任务描述（可选）"
              className="min-h-[60px] resize-y"
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
                    <p className="text-[11px] text-muted-foreground">确认后会替换当前详情输入框内容</p>
                  </div>
                  <button
                    type="button"
                    onClick={handleApplyOptimizedDescription}
                    className="rounded-md bg-primary px-2.5 py-1.5 text-xs text-primary-foreground hover:bg-primary/90"
                  >
                    替换详情
                  </button>
                </div>
                <div className="max-h-56 overflow-y-auto rounded-md border bg-background/80 p-3 text-xs whitespace-pre-wrap text-foreground">
                  {optimizePrompt.optimizedPrompt}
                </div>
              </div>
            )}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                项目 *
              </label>
              <Select
                value={selectedProjectId || null}
                onValueChange={(value) => {
                  const nextProjectId = value ?? "";
                  setSelectedProjectId(nextProjectId);
                  setCreateError(null);
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue placeholder="选择项目">
                    {(value) =>
                      typeof value === "string"
                        ? projects.find((project) => project.id === value)?.name ?? "选择项目"
                        : "选择项目"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {projects.map((project) => (
                    <SelectItem key={project.id} value={project.id}>
                      {project.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div>
              <label className="text-xs font-medium text-muted-foreground">
                优先级
              </label>
              <Select
                value={priority}
                onValueChange={(value) => setPriority(value ?? "medium")}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue placeholder="选择优先级">
                    {(value) =>
                      typeof value === "string"
                        ? PRIORITIES.find((item) => item.value === value)?.label ?? "选择优先级"
                        : "选择优先级"
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {PRIORITIES.map((item) => (
                    <SelectItem key={item.value} value={item.value}>
                      {item.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              指派给
            </label>
            <Select
              disabled={saving}
              value={assigneeId || UNASSIGNED_VALUE}
              onValueChange={(value) => {
                setCreateError(null);
                setAssigneeId(!value || value === UNASSIGNED_VALUE ? "" : value);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => {
                    if (!value || value === UNASSIGNED_VALUE) {
                      return "未指派";
                    }

                    const employee = employees.find((emp) => emp.id === value);
                    return employee ? `${employee.name} (${employee.role})` : "未指派";
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
                {employees.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({emp.role})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  图片附件
                </label>
                <p className="text-[11px] text-muted-foreground">
                  创建任务时会复制到应用托管目录，后续运行 Codex 会自动附带。
                </p>
              </div>
              <button
                type="button"
                onClick={() => void handleSelectAttachments()}
                disabled={!isTauriRuntime() || saving}
                className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
                title={isTauriRuntime() ? "选择图片" : "仅桌面端支持上传图片"}
              >
                <ImagePlus className="h-3.5 w-3.5" />
                添加图片
              </button>
            </div>

            {!isTauriRuntime() && (
              <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                当前环境不支持任务图片上传，请在桌面端使用该功能。
              </div>
            )}

            <TaskAttachmentGrid
              items={attachmentPaths.map((path) => ({
                id: path,
                name: path.split(/[\\/]/).pop() ?? path,
                path,
                removable: true,
                onRemove: () => setAttachmentPaths((current) => current.filter((item) => item !== path)),
              }))}
              emptyText="还没有添加图片"
            />
          </div>

          {createError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {createError}
            </div>
          )}

          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={() => handleOpen(false)}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent"
            >
              取消
            </button>
            <button
              onClick={handleCreate}
              disabled={!title.trim() || !selectedProjectId || saving}
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
