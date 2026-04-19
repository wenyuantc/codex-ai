import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { ImagePlus, Loader2, Sparkles } from "lucide-react";

import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { getEmployeeRoleLabel } from "@/lib/utils";
import { IMAGE_FILE_FILTERS, dedupePaths, isTauriRuntime, normalizeDialogSelection } from "@/lib/taskAttachments";
import { PRIORITIES } from "@/lib/types";
import { getCodexSettings, getRemoteCodexSettings } from "@/lib/backend";
import { getProjectWorkingDir } from "@/lib/projects";
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
  const [useWorktree, setUseWorktree] = useState("false");
  const [selectedProjectId, setSelectedProjectId] = useState(
    projectId ?? ""
  );
  const [assigneeId, setAssigneeId] = useState("");
  const [reviewerId, setReviewerId] = useState("");
  const [attachmentPaths, setAttachmentPaths] = useState<string[]>([]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [defaultsLoading, setDefaultsLoading] = useState(false);
  const [defaultAutomationEnabled, setDefaultAutomationEnabled] = useState(false);
  const selectedProject = projects.find((project) => project.id === selectedProjectId);
  const reviewerCandidates = employees.filter((employee) => employee.role === "reviewer");

  useEffect(() => {
    if (open) {
      optimizePrompt.reset();
    }
  }, [open, selectedProjectId, title, description]);

  useEffect(() => {
    if (!open) {
      return;
    }
    if (selectedProjectId && !selectedProject) {
      setDefaultsLoading(true);
      return;
    }

    let cancelled = false;
    setDefaultsLoading(true);

    const loadTaskDefaults = async () => {
      try {
        const settings =
          selectedProject?.project_type === "ssh" && selectedProject.ssh_config_id
            ? await getRemoteCodexSettings(selectedProject.ssh_config_id)
            : await getCodexSettings();
        if (cancelled) {
          return;
        }
        setDefaultAutomationEnabled(settings.task_automation_default_enabled);
        setUseWorktree(settings.git_preferences.default_task_use_worktree ? "true" : "false");
      } catch (error) {
        console.error("Failed to load task creation defaults:", error);
        if (cancelled) {
          return;
        }
        setDefaultAutomationEnabled(false);
        setUseWorktree("false");
      } finally {
        if (!cancelled) {
          setDefaultsLoading(false);
        }
      }
    };

    void loadTaskDefaults();

    return () => {
      cancelled = true;
    };
  }, [
    open,
    selectedProjectId,
    selectedProject?.id,
    selectedProject?.project_type,
    selectedProject?.ssh_config_id,
  ]);

  const handleOpen = (isOpen: boolean) => {
    if (isOpen) {
      fetchEmployees();
      fetchProjects();
      setDefaultsLoading(true);
      setTitle("");
      setDescription("");
      setPriority("medium");
      setUseWorktree("false");
      setSelectedProjectId(projectId ?? "");
      setAssigneeId("");
      setReviewerId("");
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
      projectRepoPath: getProjectWorkingDir(selectedProject),
      title,
      description,
      currentPrompt: null,
      taskTitle: null,
      sessionSummary: null,
      taskId: null,
      workingDir: getProjectWorkingDir(selectedProject),
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
    if (defaultsLoading) {
      setCreateError("任务默认设置仍在加载，请稍候再试。");
      return;
    }
    if (defaultAutomationEnabled && !reviewerId) {
      setCreateError("当前已开启“新建任务默认自动质控”，请先指定审查员。");
      return;
    }
    setCreateError(null);
    setSaving(true);
    try {
      await createTask({
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        project_id: selectedProjectId,
        use_worktree: useWorktree === "true",
        assignee_id: assigneeId || undefined,
        reviewer_id: reviewerId || undefined,
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
      <DialogContent className="max-w-4xl">
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

          <div className="space-y-3">
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

            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  Worktree 模式
                </label>
                <Select
                  value={useWorktree}
                  onValueChange={(value) => setUseWorktree(value ?? "false")}
                  disabled={saving || defaultsLoading}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder="选择是否启用 worktree">
                      {(value) => {
                        if (value === "true") {
                          return "是";
                        }

                        if (value === "false") {
                          return "否";
                        }

                        return "选择是否启用 worktree";
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="false">否</SelectItem>
                    <SelectItem value="true">是</SelectItem>
                  </SelectContent>
                </Select>
                <p className="mt-1 text-[11px] text-muted-foreground">
                  {defaultsLoading
                    ? "正在加载当前项目的默认设置…"
                    : "默认直接使用项目工作目录；开启后会为该任务准备独立 worktree。"}
                </p>
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
                    return employee
                      ? `${employee.name} (${getEmployeeRoleLabel(employee.role)})`
                      : "未指派";
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
                {employees.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({getEmployeeRoleLabel(emp.role)})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              审查员
            </label>
            <Select
              disabled={saving}
              value={reviewerId || UNASSIGNED_VALUE}
              onValueChange={(value) => {
                setCreateError(null);
                setReviewerId(!value || value === UNASSIGNED_VALUE ? "" : value);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => {
                    if (!value || value === UNASSIGNED_VALUE) {
                      return "未指定";
                    }

                    const employee = reviewerCandidates.find((emp) => emp.id === value);
                    return employee
                      ? `${employee.name} (${getEmployeeRoleLabel(employee.role)})`
                      : "未指定";
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
                {reviewerCandidates.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({getEmployeeRoleLabel(emp.role)})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {defaultAutomationEnabled && (
              <p className="mt-1 text-[11px] text-muted-foreground">
                当前已开启“新建任务默认自动质控”，新建任务时需要指定审查员。
              </p>
            )}
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
              disabled={!title.trim() || !selectedProjectId || saving || defaultsLoading}
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? "创建中..." : defaultsLoading ? "加载默认值..." : "创建"}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
