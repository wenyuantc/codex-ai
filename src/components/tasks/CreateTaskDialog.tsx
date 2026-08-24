import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Loader2, Paperclip, Play, Sparkles, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useAiOptimizePrompt } from "@/hooks/useAiOptimizePrompt";
import { getEmployeeRoleLabel, getPriorityLabel } from "@/lib/utils";
import { dedupePaths, isTauriRuntime, normalizeDialogSelection } from "@/lib/taskAttachments";
import { formatEmployeeAiProviderLabel } from "@/lib/types";
import type { CodexSessionKind, Milestone, Tag, Task } from "@/lib/types";
import { PRIORITIES } from "@/lib/types";
import {
  addTaskDependency,
  getCodexSettings,
  getRemoteCodexSettings,
  listMilestones,
  listTags,
  listTasks,
  setTaskTags,
} from "@/lib/backend";
import { getProjectWorkingDir } from "@/lib/projects";
import { createTaskForRun, continueCreatedTaskRun } from "@/lib/taskCreateAndRun";
import { useTaskBackgroundRunStore } from "@/stores/taskBackgroundRunStore";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TaskAttachmentGrid } from "./TaskAttachmentGrid";

const UNASSIGNED_VALUE = "__unassigned__";
const NONE_VALUE = "__none__";

interface CreateTaskDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId?: string;
  onOpenLog?: (taskId: string, sessionKind?: CodexSessionKind) => void;
  /** Fired after a successful create (and tags/deps attach) so boards can refresh maps. */
  onCreated?: (task: Task) => void;
}

export function CreateTaskDialog({
  open,
  onOpenChange,
  projectId,
  onOpenLog,
  onCreated,
}: CreateTaskDialogProps) {
  const { t } = useTranslation(["tasks", "common", "kanban"]);
  const { createTask } = useTaskStore();
  const { projects, fetchProjects } = useProjectStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const optimizePrompt = useAiOptimizePrompt(open);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("medium");
  const [useWorktree, setUseWorktree] = useState("false");
  const [selectedProjectId, setSelectedProjectId] = useState(projectId ?? "");
  const [assigneeId, setAssigneeId] = useState("");
  const [reviewerId, setReviewerId] = useState("");
  const [coordinatorId, setCoordinatorId] = useState("");
  const [dueDate, setDueDate] = useState("");
  const [milestoneId, setMilestoneId] = useState("");
  const [projectMilestones, setProjectMilestones] = useState<Milestone[]>([]);
  const [projectTags, setProjectTags] = useState<Tag[]>([]);
  const [projectTasks, setProjectTasks] = useState<Task[]>([]);
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [dependencyTaskIds, setDependencyTaskIds] = useState<string[]>([]);
  const [attachmentPaths, setAttachmentPaths] = useState<string[]>([]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [defaultsLoading, setDefaultsLoading] = useState(false);
  const [defaultAutomationEnabled, setDefaultAutomationEnabled] = useState(false);
  const busy = saving;
  const setBackgroundPhase = useTaskBackgroundRunStore((state) => state.setPhase);
  const selectedProject = projects.find((project) => project.id === selectedProjectId);
  const coordinatorCandidates = employees.filter((employee) => employee.role === "coordinator");
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

  useEffect(() => {
    if (!open || !selectedProjectId) {
      setProjectTags([]);
      setProjectMilestones([]);
      setProjectTasks([]);
      return;
    }

    let cancelled = false;
    void listTags(selectedProjectId)
      .then((tags) => {
        if (!cancelled) {
          setProjectTags(tags);
        }
      })
      .catch((error) => {
        console.error("Failed to load project tags:", error);
        if (!cancelled) {
          setProjectTags([]);
        }
      });
    void listMilestones(selectedProjectId)
      .then((items) => {
        if (!cancelled) {
          setProjectMilestones(items);
        }
      })
      .catch((error) => {
        console.error("Failed to load project milestones:", error);
        if (!cancelled) {
          setProjectMilestones([]);
        }
      });
    // Dependency picker only. fetchTasks(selectedProjectId) replaces the board cache
    // and pins activeProjectId, hiding other projects when the header is "all".
    void listTasks({ projectId: selectedProjectId })
      .then((rows) => {
        if (!cancelled) {
          setProjectTasks(rows.filter((task) => task.status !== "archived"));
        }
      })
      .catch((error) => {
        console.error("Failed to load project tasks:", error);
        if (!cancelled) {
          setProjectTasks([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open, selectedProjectId]);

  const handleOpen = (isOpen: boolean) => {
    // Block dismiss while create is in flight. Successful submits close via closeAfterSuccess().
    if (!isOpen && saving) {
      return;
    }
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
      setCoordinatorId("");
      setDueDate("");
      setMilestoneId("");
      setSelectedTagIds([]);
      setDependencyTaskIds([]);
      setAttachmentPaths([]);
      setCreateError(null);
    }
    onOpenChange(isOpen);
  };

  /** Close without the in-flight guard (create / create-and-run success only). */
  const closeAfterSuccess = () => {
    setSaving(false);
    onOpenChange(false);
  };

  const buildCreatePayload = () => ({
    title: title.trim(),
    description: description.trim() || undefined,
    priority,
    project_id: selectedProjectId,
    use_worktree: useWorktree === "true",
    assignee_id: assigneeId || undefined,
    reviewer_id: reviewerId || undefined,
    coordinator_id: coordinatorId || undefined,
    due_date: dueDate || null,
    milestone_id: milestoneId || null,
    attachment_source_paths: attachmentPaths,
  });

  const validateCreateBase = (requireAssignee: boolean): string | null => {
    if (!title.trim() || !selectedProjectId) {
      return t("createDialog.errors.titleAndProjectRequired");
    }
    if (defaultsLoading) {
      return t("createDialog.errors.defaultsStillLoading");
    }
    if (defaultAutomationEnabled && !reviewerId) {
      return t("createDialog.errors.automationRequiresReviewer");
    }
    if (requireAssignee && !assigneeId) {
      return t("createDialog.errors.createAndRunRequiresAssignee");
    }
    return null;
  };

  const handleSelectAttachments = async () => {
    const selected = await openFileDialog({
      directory: false,
      multiple: true,
      title: t("createDialog.fileDialogTitle"),
      filters: [
        {
          name: t("createDialog.fileDialogFilterName"),
          extensions: [
            "png",
            "jpg",
            "jpeg",
            "gif",
            "webp",
            "bmp",
            "svg",
            "pdf",
            "md",
            "txt",
            "log",
            "json",
            "csv",
            "doc",
            "docx",
            "zip",
          ],
        },
      ],
    });

    const nextPaths = dedupePaths([...attachmentPaths, ...normalizeDialogSelection(selected)]);
    setAttachmentPaths(nextPaths);
  };

  const handleGenerateOptimizedDescription = async () => {
    if (!selectedProjectId) {
      optimizePrompt.showError(t("createDialog.errors.projectMissingForOptimize"));
      return;
    }

    if (!selectedProject) {
      optimizePrompt.showError(t("createDialog.errors.projectNotFoundForOptimize"));
      return;
    }

    await optimizePrompt.generate({
      scene: "task_create",
      projectId: selectedProject.id,
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
    const validationError = validateCreateBase(false);
    if (validationError) {
      setCreateError(validationError);
      return;
    }
    setCreateError(null);
    setSaving(true);
    try {
      const created = await createTask(buildCreatePayload(), {
        refreshProjectId: projectId,
      });
      if (selectedTagIds.length > 0) {
        await setTaskTags({ task_id: created.id, tag_ids: selectedTagIds });
      }
      for (const dependsOnTaskId of dependencyTaskIds) {
        await addTaskDependency({
          task_id: created.id,
          depends_on_task_id: dependsOnTaskId,
        });
      }
      onCreated?.(created);
      closeAfterSuccess();
    } catch (error) {
      setCreateError(error instanceof Error ? error.message : String(error));
      setSaving(false);
    }
  };

  const handleCreateAndRun = async () => {
    const validationError = validateCreateBase(true);
    if (validationError) {
      setCreateError(validationError);
      return;
    }
    if (!selectedProject) {
      setCreateError(t("createDialog.errors.projectNotFoundForRun"));
      return;
    }
    const assignee = employees.find((employee) => employee.id === assigneeId);
    if (!assignee) {
      setCreateError(t("createDialog.errors.assigneeNotFound"));
      return;
    }

    setCreateError(null);
    setSaving(true);
    const payload = buildCreatePayload();
    const projectSnapshot = selectedProject;
    const assigneeSnapshot = assignee;
    const openLog = onOpenLog;

    try {
      // 1) Create only — keep dialog open for create errors.
      const task = await createTaskForRun({
        payload,
        tagIds: selectedTagIds,
        dependencyTaskIds,
        refreshProjectId: projectId,
      });

      // 2) Seed kanban badge immediately, close dialog, run plan+execute in background.
      setBackgroundPhase(task.id, payload.coordinator_id ? "planning" : "starting");
      onCreated?.(task);
      closeAfterSuccess();

      void continueCreatedTaskRun({
        task,
        payload,
        project: projectSnapshot,
        assignee: assigneeSnapshot,
      })
        .then(({ task: startedTask }) => {
          openLog?.(startedTask.id, "execution");
        })
        .catch((error) => {
          console.error("Background create-and-run failed:", error);
        });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setCreateError(message);
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpen}>
      <DialogContent className="max-h-[min(92vh,calc(100vh-2rem))] w-[min(96vw,64rem)] max-w-[min(96vw,64rem)] overflow-y-auto sm:max-w-[min(96vw,64rem)]">
        <DialogHeader>
          <DialogTitle>{t("createDialog.title")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("createDialog.fields.title")}
            </label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t("createDialog.fields.titlePlaceholder")}
              className="mt-1"
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <label className="text-xs font-medium text-muted-foreground">
                {t("createDialog.fields.description")}
              </label>
              <button
                type="button"
                onClick={() => void handleGenerateOptimizedDescription()}
                disabled={busy || optimizePrompt.loading}
                className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
              >
                {optimizePrompt.loading ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Sparkles className="h-3.5 w-3.5" />
                )}
                {t("createDialog.aiOptimize")}
              </button>
            </div>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("createDialog.fields.descriptionPlaceholder")}
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
                    <p className="text-xs font-medium text-primary">
                      {t("createDialog.optimizedPromptTitle")}
                    </p>
                    <p className="text-[11px] text-muted-foreground">
                      {t("createDialog.optimizedPromptHint")}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={handleApplyOptimizedDescription}
                    className="rounded-md bg-primary px-2.5 py-1.5 text-xs text-primary-foreground hover:bg-primary/90"
                  >
                    {t("createDialog.replaceDescription")}
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
                {t("createDialog.fields.project")}
              </label>
              <Select
                value={selectedProjectId || null}
                onValueChange={(value) => {
                  const nextProjectId = value ?? "";
                  setSelectedProjectId(nextProjectId);
                  setSelectedTagIds([]);
                  setDependencyTaskIds([]);
                  setMilestoneId("");
                  setCreateError(null);
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue placeholder={t("createDialog.fields.projectPlaceholder")}>
                    {(value) =>
                      typeof value === "string"
                        ? (projects.find((project) => project.id === value)?.name ??
                          t("createDialog.fields.projectPlaceholder"))
                        : t("createDialog.fields.projectPlaceholder")
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
                  {t("createDialog.fields.worktreeMode")}
                </label>
                <Select
                  value={useWorktree}
                  onValueChange={(value) => setUseWorktree(value ?? "false")}
                  disabled={busy || defaultsLoading}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder={t("createDialog.fields.worktreePlaceholder")}>
                      {(value) => {
                        if (value === "true") {
                          return t("common:yes");
                        }

                        if (value === "false") {
                          return t("common:no");
                        }

                        return t("createDialog.fields.worktreePlaceholder");
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="false">{t("common:no")}</SelectItem>
                    <SelectItem value="true">{t("common:yes")}</SelectItem>
                  </SelectContent>
                </Select>
                <p className="mt-1 text-[11px] text-muted-foreground">
                  {defaultsLoading
                    ? t("createDialog.worktreeLoadingHint")
                    : t("createDialog.worktreeHint")}
                </p>
              </div>

              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("createDialog.fields.priority")}
                </label>
                <Select value={priority} onValueChange={(value) => setPriority(value ?? "medium")}>
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder={t("createDialog.fields.priorityPlaceholder")}>
                      {(value) =>
                        typeof value === "string"
                          ? getPriorityLabel(value)
                          : t("createDialog.fields.priorityPlaceholder")
                      }
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {PRIORITIES.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {getPriorityLabel(item.value)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("createDialog.fields.dueDate")}
                </label>
                <Input
                  type="date"
                  value={dueDate}
                  onChange={(e) => setDueDate(e.target.value)}
                  className="mt-1"
                  disabled={busy}
                />
              </div>

              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("createDialog.fields.milestone")}
                </label>
                <Select
                  value={milestoneId || NONE_VALUE}
                  onValueChange={(value) =>
                    setMilestoneId(!value || value === NONE_VALUE ? "" : value)
                  }
                  disabled={busy || !selectedProjectId}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder={t("createDialog.fields.noMilestone")}>
                      {(value) => {
                        if (!value || value === NONE_VALUE) {
                          return t("createDialog.fields.noMilestone");
                        }
                        return (
                          projectMilestones.find((item) => item.id === value)?.name ??
                          t("createDialog.fields.noMilestone")
                        );
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={NONE_VALUE}>
                      {t("createDialog.fields.noMilestone")}
                    </SelectItem>
                    {projectMilestones.map((item) => (
                      <SelectItem key={item.id} value={item.id}>
                        {item.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground">
                {t("createDialog.fields.tags")}
              </label>
              <div className="flex flex-wrap gap-1.5">
                {selectedTagIds.length === 0 && (
                  <span className="text-[11px] text-muted-foreground">
                    {t("createDialog.fields.noTagsSelected")}
                  </span>
                )}
                {selectedTagIds.map((tagId) => {
                  const tag = projectTags.find((item) => item.id === tagId);
                  if (!tag) {
                    return null;
                  }
                  return (
                    <Badge key={tag.id} variant="secondary" className="gap-1 pr-1">
                      {tag.name}
                      <button
                        type="button"
                        onClick={() =>
                          setSelectedTagIds((current) => current.filter((id) => id !== tag.id))
                        }
                        className="rounded-full p-0.5 hover:bg-muted"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </Badge>
                  );
                })}
              </div>
              <Select
                disabled={busy || !selectedProjectId || projectTags.length === 0}
                value={NONE_VALUE}
                onValueChange={(value) => {
                  if (!value || value === NONE_VALUE || selectedTagIds.includes(value)) {
                    return;
                  }
                  setSelectedTagIds((current) => [...current, value]);
                }}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue placeholder={t("createDialog.fields.selectTag")}>
                    {t("createDialog.fields.selectTag")}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {projectTags
                    .filter((tag) => !selectedTagIds.includes(tag.id))
                    .map((tag) => (
                      <SelectItem key={tag.id} value={tag.id}>
                        {tag.name}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground">
                {t("createDialog.fields.dependencies")}
              </label>
              <div className="flex flex-wrap gap-1.5">
                {dependencyTaskIds.length === 0 && (
                  <span className="text-[11px] text-muted-foreground">
                    {t("createDialog.fields.noDependenciesSelected")}
                  </span>
                )}
                {dependencyTaskIds.map((taskId) => {
                  const depTask = projectTasks.find((item) => item.id === taskId);
                  return (
                    <Badge key={taskId} variant="outline" className="gap-1 pr-1">
                      {depTask?.title ?? taskId}
                      <button
                        type="button"
                        onClick={() =>
                          setDependencyTaskIds((current) => current.filter((id) => id !== taskId))
                        }
                        className="rounded-full p-0.5 hover:bg-muted"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </Badge>
                  );
                })}
              </div>
              <Select
                disabled={busy || !selectedProjectId || projectTasks.length === 0}
                value={NONE_VALUE}
                onValueChange={(value) => {
                  if (!value || value === NONE_VALUE || dependencyTaskIds.includes(value)) {
                    return;
                  }
                  setDependencyTaskIds((current) => [...current, value]);
                }}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue placeholder={t("createDialog.fields.selectDependency")}>
                    {t("createDialog.fields.selectDependency")}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {projectTasks
                    .filter((item) => !dependencyTaskIds.includes(item.id))
                    .map((item) => (
                      <SelectItem key={item.id} value={item.id}>
                        {item.title}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("createDialog.fields.assignee")}
            </label>
            <Select
              disabled={busy}
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
                      return t("createDialog.fields.unspecified");
                    }

                    const employee = employees.find((emp) => emp.id === value);
                    return employee
                      ? `${employee.name} (${getEmployeeRoleLabel(employee.role)}) · ${formatEmployeeAiProviderLabel(employee.ai_provider)}`
                      : t("createDialog.fields.unspecified");
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>
                  {t("createDialog.fields.unspecified")}
                </SelectItem>
                {employees.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({getEmployeeRoleLabel(emp.role)}) ·{" "}
                    {formatEmployeeAiProviderLabel(emp.ai_provider)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("createDialog.fields.reviewer")}
            </label>
            <Select
              disabled={busy}
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
                      return t("createDialog.fields.unspecified");
                    }

                    const employee = reviewerCandidates.find((emp) => emp.id === value);
                    return employee
                      ? `${employee.name} (${getEmployeeRoleLabel(employee.role)}) · ${formatEmployeeAiProviderLabel(employee.ai_provider)}`
                      : t("createDialog.fields.unspecified");
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>
                  {t("createDialog.fields.unspecified")}
                </SelectItem>
                {reviewerCandidates.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({getEmployeeRoleLabel(emp.role)}) ·{" "}
                    {formatEmployeeAiProviderLabel(emp.ai_provider)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {defaultAutomationEnabled && (
              <p className="mt-1 text-[11px] text-muted-foreground">
                {t("createDialog.defaultAutomationReviewerHint")}
              </p>
            )}
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("createDialog.fields.coordinator")}
            </label>
            <Select
              disabled={busy}
              value={coordinatorId || UNASSIGNED_VALUE}
              onValueChange={(value) => {
                setCreateError(null);
                setCoordinatorId(!value || value === UNASSIGNED_VALUE ? "" : value);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => {
                    if (!value || value === UNASSIGNED_VALUE) {
                      return t("createDialog.fields.unspecified");
                    }

                    const employee = coordinatorCandidates.find((emp) => emp.id === value);
                    return employee
                      ? `${employee.name} (${getEmployeeRoleLabel(employee.role)}) · ${formatEmployeeAiProviderLabel(employee.ai_provider)}`
                      : t("createDialog.fields.unspecified");
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNASSIGNED_VALUE}>
                  {t("createDialog.fields.unspecified")}
                </SelectItem>
                {coordinatorCandidates.map((emp) => (
                  <SelectItem key={emp.id} value={emp.id}>
                    {emp.name} ({getEmployeeRoleLabel(emp.role)}) ·{" "}
                    {formatEmployeeAiProviderLabel(emp.ai_provider)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("createDialog.fields.attachments")}
                </label>
                <p className="text-[11px] text-muted-foreground">
                  {t("createDialog.attachmentsHint")}
                </p>
              </div>
              <button
                type="button"
                onClick={() => void handleSelectAttachments()}
                disabled={!isTauriRuntime() || saving}
                className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
                title={
                  isTauriRuntime()
                    ? t("createDialog.selectAttachmentTitle")
                    : t("createDialog.attachmentDesktopOnlyTooltip")
                }
              >
                <Paperclip className="h-3.5 w-3.5" />
                {t("createDialog.addAttachment")}
              </button>
            </div>

            {!isTauriRuntime() && (
              <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                {t("createDialog.attachmentUnsupported")}
              </div>
            )}

            <TaskAttachmentGrid
              items={attachmentPaths.map((path) => ({
                id: path,
                name: path.split(/[\\/]/).pop() ?? path,
                path,
                removable: true,
                onRemove: () =>
                  setAttachmentPaths((current) => current.filter((item) => item !== path)),
              }))}
              emptyText={t("createDialog.noAttachmentsYet")}
            />
          </div>

          {createError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {createError}
            </div>
          )}

          <div className="flex flex-wrap justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={() => handleOpen(false)}
              disabled={busy}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent disabled:opacity-50"
            >
              {t("common:cancel")}
            </button>
            <button
              type="button"
              onClick={() => void handleCreate()}
              disabled={!title.trim() || !selectedProjectId || busy || defaultsLoading}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent disabled:opacity-50"
            >
              {saving
                ? t("createDialog.creating")
                : defaultsLoading
                  ? t("createDialog.loadingDefaults")
                  : t("common:create")}
            </button>
            <button
              type="button"
              onClick={() => void handleCreateAndRun()}
              disabled={!title.trim() || !selectedProjectId || busy || defaultsLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {busy ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Play className="h-3.5 w-3.5" />
              )}
              {defaultsLoading
                ? t("createDialog.loadingDefaults")
                : busy
                  ? t("createDialog.creating")
                  : t("createDialog.createAndRun")}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
