import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { LayoutTemplate, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
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
import { Textarea } from "@/components/ui/textarea";
import {
  applyTaskTemplate,
  createTaskTemplate,
  deleteTaskTemplate,
  listTaskTemplates,
  updateTaskTemplate,
} from "@/lib/backend";
import type { TaskTemplate, TaskTemplateSubtaskSpec } from "@/lib/types";
import { PRIORITIES } from "@/lib/types";
import { formatDate, getEmployeeRoleLabel, getPriorityLabel } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";

const GLOBAL_VALUE = "__global__";
const UNASSIGNED_VALUE = "__unassigned__";
const TEMPLATE_VAR_RE = /\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}/g;

function extractTemplateVariables(...texts: Array<string | null | undefined>): string[] {
  const result: string[] = [];
  for (const text of texts) {
    if (!text) {
      continue;
    }
    for (const match of text.matchAll(TEMPLATE_VAR_RE)) {
      const name = match[1];
      if (name && !result.includes(name)) {
        result.push(name);
      }
    }
  }
  return result;
}

interface TemplateFormState {
  name: string;
  description: string;
  projectId: string;
  titleTemplate: string;
  descriptionTemplate: string;
  priority: string;
  useWorktree: boolean;
  tags: string[];
  subtasks: string[];
}

function emptyForm(projectId?: string | null): TemplateFormState {
  return {
    name: "",
    description: "",
    projectId: projectId ?? "",
    titleTemplate: "",
    descriptionTemplate: "",
    priority: "medium",
    useWorktree: false,
    tags: [],
    subtasks: [],
  };
}

function formFromTemplate(template: TaskTemplate): TemplateFormState {
  return {
    name: template.name,
    description: template.description ?? "",
    projectId: template.project_id ?? "",
    titleTemplate: template.title_template,
    descriptionTemplate: template.description_template ?? "",
    priority: template.priority || "medium",
    useWorktree: template.use_worktree,
    tags: [...template.tags],
    subtasks: template.subtasks.map((item) => item.title),
  };
}

interface TaskTemplateManagerDialogProps {
  open: boolean;
  projectId?: string | null;
  onOpenChange: (open: boolean) => void;
  onApplied?: () => void;
}

export function TaskTemplateManagerDialog({
  open,
  projectId,
  onOpenChange,
  onApplied,
}: TaskTemplateManagerDialogProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const projects = useProjectStore((state) => state.projects);
  const employees = useEmployeeStore((state) => state.employees);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);
  const [templates, setTemplates] = useState<TaskTemplate[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<TemplateFormState>(() => emptyForm(projectId));
  const [tagDraft, setTagDraft] = useState("");
  const [subtaskDraft, setSubtaskDraft] = useState("");
  const [mode, setMode] = useState<"edit" | "apply">("edit");
  const [variableRows, setVariableRows] = useState<Record<string, string>[]>([{}]);
  const [assigneeId, setAssigneeId] = useState("");
  const [reviewerId, setReviewerId] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [applying, setApplying] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === selectedId) ?? null,
    [templates, selectedId],
  );
  const variables = useMemo(
    () =>
      extractTemplateVariables(
        selectedTemplate?.title_template,
        selectedTemplate?.description_template,
      ),
    [selectedTemplate],
  );

  const projectEmployees = useMemo(() => {
    if (!projectId) {
      return employees;
    }
    return employees.filter(
      (employee) => !employee.project_id || employee.project_id === projectId,
    );
  }, [employees, projectId]);
  const reviewers = projectEmployees.filter((employee) => employee.role === "reviewer");

  const loadTemplates = useCallback(async () => {
    setLoading(true);
    try {
      const items = await listTaskTemplates(projectId ?? null);
      setTemplates(items);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    if (!open) {
      return;
    }
    setMode("edit");
    setSelectedId(null);
    setForm(emptyForm(projectId));
    setTagDraft("");
    setSubtaskDraft("");
    setVariableRows([{}]);
    setAssigneeId("");
    setReviewerId("");
    setConfirmDelete(false);
    setError(null);
    setNotice(null);
    void loadTemplates();
    void fetchEmployees();
  }, [open, projectId, loadTemplates, fetchEmployees]);

  const selectTemplate = (template: TaskTemplate) => {
    setSelectedId(template.id);
    setForm(formFromTemplate(template));
    setMode("edit");
    setConfirmDelete(false);
    setError(null);
    setNotice(null);
    setVariableRows([{}]);
  };

  const startNew = () => {
    setSelectedId(null);
    setForm(emptyForm(projectId));
    setMode("edit");
    setConfirmDelete(false);
    setError(null);
    setNotice(null);
  };

  const addListItem = (
    field: "tags" | "subtasks",
    draft: string,
    setDraft: (value: string) => void,
  ) => {
    const value = draft.trim();
    if (!value) {
      return;
    }
    setForm((current) =>
      current[field].includes(value)
        ? current
        : { ...current, [field]: [...current[field], value] },
    );
    setDraft("");
  };

  const handleSave = async () => {
    if (!form.name.trim() || !form.titleTemplate.trim()) {
      setError(t("templates.needNameAndTitle"));
      return;
    }
    setSaving(true);
    setError(null);
    setNotice(null);
    const subtasks: TaskTemplateSubtaskSpec[] = form.subtasks.map((title, index) => ({
      title,
      sort_order: index + 1,
    }));
    const payload = {
      name: form.name.trim(),
      description: form.description.trim() || null,
      project_id: form.projectId.trim() || null,
      title_template: form.titleTemplate.trim(),
      description_template: form.descriptionTemplate.trim() || null,
      priority: form.priority,
      use_worktree: form.useWorktree,
      tags: form.tags,
      subtasks,
    };
    try {
      const saved = selectedId
        ? await updateTaskTemplate(selectedId, payload)
        : await createTaskTemplate(payload);
      await loadTemplates();
      setSelectedId(saved.id);
      setForm(formFromTemplate(saved));
      setNotice(t("templates.saved"));
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!selectedId) {
      return;
    }
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await deleteTaskTemplate(selectedId);
      await loadTemplates();
      startNew();
      setNotice(t("templates.deleted"));
    } catch (deleteError) {
      setError(deleteError instanceof Error ? deleteError.message : String(deleteError));
    } finally {
      setSaving(false);
    }
  };

  const handleApply = async () => {
    if (!selectedId) {
      setError(t("templates.needSelected"));
      return;
    }
    if (!projectId) {
      setError(t("templates.needProject"));
      return;
    }
    setApplying(true);
    setError(null);
    setNotice(null);
    try {
      await applyTaskTemplate({
        template_id: selectedId,
        project_id: projectId,
        variable_sets: variables.length === 0 ? [] : variableRows,
        assignee_id: assigneeId || null,
        reviewer_id: reviewerId || null,
      });
      onApplied?.();
      onOpenChange(false);
    } catch (applyError) {
      setError(applyError instanceof Error ? applyError.message : String(applyError));
    } finally {
      setApplying(false);
    }
  };

  const updateRow = (index: number, key: string, value: string) => {
    setVariableRows((current) =>
      current.map((row, rowIndex) => (rowIndex === index ? { ...row, [key]: value } : row)),
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(92vh,calc(100vh-2rem))] w-[min(96vw,72rem)] max-w-[min(96vw,72rem)] flex-col overflow-hidden sm:max-w-[min(96vw,72rem)]">
        <DialogHeader>
          <DialogTitle>{t("templates.managerTitle")}</DialogTitle>
          <p className="text-xs text-muted-foreground">{t("templates.managerDescription")}</p>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 gap-4 overflow-hidden md:grid-cols-[16rem_1fr]">
          <div className="flex min-h-0 flex-col gap-2 border-r border-border pr-3">
            <Button variant="outline" size="sm" onClick={startNew}>
              <Plus className="h-4 w-4" />
              {t("templates.newTemplate")}
            </Button>
            <div className="min-h-0 flex-1 space-y-1 overflow-y-auto">
              {loading && (
                <p className="px-1 text-xs text-muted-foreground">{t("common:loading")}</p>
              )}
              {!loading && templates.length === 0 && (
                <p className="px-1 text-xs text-muted-foreground">{t("templates.emptyList")}</p>
              )}
              {templates.map((template) => {
                const projectName = template.project_id
                  ? (projects.find((project) => project.id === template.project_id)?.name ??
                    template.project_id)
                  : t("templates.global");
                return (
                  <button
                    key={template.id}
                    type="button"
                    onClick={() => selectTemplate(template)}
                    className={`w-full rounded-md px-2 py-2 text-left hover:bg-accent ${
                      selectedId === template.id ? "bg-accent" : ""
                    }`}
                  >
                    <div className="truncate text-sm font-medium">{template.name}</div>
                    <div className="truncate text-[11px] text-muted-foreground">{projectName}</div>
                    <div className="text-[11px] text-muted-foreground">
                      {formatDate(template.updated_at)}
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="min-h-0 overflow-y-auto pr-1">
            {mode === "apply" ? (
              <div className="space-y-3">
                <div>
                  <h3 className="text-sm font-medium">{t("templates.applyTitle")}</h3>
                  <p className="text-xs text-muted-foreground">{t("templates.applyHint")}</p>
                </div>
                {variables.length === 0 ? (
                  <p className="text-xs text-muted-foreground">{t("templates.noVariables")}</p>
                ) : (
                  <div className="space-y-2">
                    <div className="overflow-x-auto rounded-md border">
                      <table className="w-full text-xs">
                        <thead className="bg-muted/50">
                          <tr>
                            {variables.map((variable) => (
                              <th key={variable} className="px-2 py-1.5 text-left font-medium">
                                {variable}
                              </th>
                            ))}
                            <th className="w-10" />
                          </tr>
                        </thead>
                        <tbody>
                          {variableRows.map((row, index) => (
                            <tr key={index} className="border-t">
                              {variables.map((variable) => (
                                <td key={variable} className="px-2 py-1.5">
                                  <Input
                                    value={row[variable] ?? ""}
                                    onChange={(event) =>
                                      updateRow(index, variable, event.target.value)
                                    }
                                  />
                                </td>
                              ))}
                              <td className="px-1">
                                <Button
                                  variant="ghost"
                                  size="icon-sm"
                                  onClick={() =>
                                    setVariableRows((current) =>
                                      current.length <= 1
                                        ? current
                                        : current.filter((_, rowIndex) => rowIndex !== index),
                                    )
                                  }
                                  aria-label={t("templates.removeRow")}
                                >
                                  <Trash2 className="h-3.5 w-3.5" />
                                </Button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setVariableRows((current) => [...current, {}])}
                      disabled={variableRows.length >= 100}
                    >
                      <Plus className="h-4 w-4" />
                      {t("templates.addRow")}
                    </Button>
                  </div>
                )}
                <div className="grid gap-3 md:grid-cols-2">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground">
                      {t("templates.assignee")}
                    </label>
                    <Select
                      value={assigneeId || UNASSIGNED_VALUE}
                      onValueChange={(value) =>
                        setAssigneeId(!value || value === UNASSIGNED_VALUE ? "" : value)
                      }
                    >
                      <SelectTrigger className="mt-1 bg-background">
                        <SelectValue>
                          {(value) =>
                            !value || value === UNASSIGNED_VALUE
                              ? t("templates.unspecified")
                              : (projectEmployees.find((item) => item.id === value)?.name ??
                                t("templates.unspecified"))
                          }
                        </SelectValue>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={UNASSIGNED_VALUE}>
                          {t("templates.unspecified")}
                        </SelectItem>
                        {projectEmployees.map((employee) => (
                          <SelectItem key={employee.id} value={employee.id}>
                            {employee.name} ({getEmployeeRoleLabel(employee.role)})
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground">
                      {t("templates.reviewer")}
                    </label>
                    <Select
                      value={reviewerId || UNASSIGNED_VALUE}
                      onValueChange={(value) =>
                        setReviewerId(!value || value === UNASSIGNED_VALUE ? "" : value)
                      }
                    >
                      <SelectTrigger className="mt-1 bg-background">
                        <SelectValue>
                          {(value) =>
                            !value || value === UNASSIGNED_VALUE
                              ? t("templates.unspecified")
                              : (reviewers.find((item) => item.id === value)?.name ??
                                t("templates.unspecified"))
                          }
                        </SelectValue>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={UNASSIGNED_VALUE}>
                          {t("templates.unspecified")}
                        </SelectItem>
                        {reviewers.map((employee) => (
                          <SelectItem key={employee.id} value={employee.id}>
                            {employee.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.name")}
                  </label>
                  <Input
                    className="mt-1"
                    value={form.name}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, name: event.target.value }))
                    }
                    placeholder={t("templates.namePlaceholder")}
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.description")}
                  </label>
                  <Textarea
                    className="mt-1 min-h-[60px] resize-y"
                    value={form.description}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, description: event.target.value }))
                    }
                    placeholder={t("templates.descriptionPlaceholder")}
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.bindProject")}
                  </label>
                  <Select
                    value={form.projectId || GLOBAL_VALUE}
                    onValueChange={(value) =>
                      setForm((current) => ({
                        ...current,
                        projectId: !value || value === GLOBAL_VALUE ? "" : value,
                      }))
                    }
                  >
                    <SelectTrigger className="mt-1 bg-background">
                      <SelectValue>
                        {(value) =>
                          !value || value === GLOBAL_VALUE
                            ? t("templates.bindGlobal")
                            : (projects.find((project) => project.id === value)?.name ??
                              t("templates.bindGlobal"))
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value={GLOBAL_VALUE}>{t("templates.bindGlobal")}</SelectItem>
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
                    {t("templates.titleTemplate")}
                  </label>
                  <Input
                    className="mt-1"
                    value={form.titleTemplate}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, titleTemplate: event.target.value }))
                    }
                    placeholder={t("templates.titleTemplatePlaceholder")}
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.descriptionTemplate")}
                  </label>
                  <Textarea
                    className="mt-1 min-h-[60px] resize-y"
                    value={form.descriptionTemplate}
                    onChange={(event) =>
                      setForm((current) => ({
                        ...current,
                        descriptionTemplate: event.target.value,
                      }))
                    }
                    placeholder={t("templates.descriptionTemplatePlaceholder")}
                  />
                  <p className="mt-1 text-[11px] text-muted-foreground">
                    {t("templates.variableHint")}
                  </p>
                </div>
                <div className="grid gap-3 md:grid-cols-2">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground">
                      {t("templates.priority")}
                    </label>
                    <Select
                      value={form.priority}
                      onValueChange={(value) =>
                        setForm((current) => ({ ...current, priority: value ?? "medium" }))
                      }
                    >
                      <SelectTrigger className="mt-1 bg-background">
                        <SelectValue>
                          {(value) =>
                            typeof value === "string"
                              ? getPriorityLabel(value)
                              : t("templates.priority")
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
                  <div>
                    <label className="text-xs font-medium text-muted-foreground">
                      {t("templates.worktree")}
                    </label>
                    <Select
                      value={form.useWorktree ? "true" : "false"}
                      onValueChange={(value) =>
                        setForm((current) => ({ ...current, useWorktree: value === "true" }))
                      }
                    >
                      <SelectTrigger className="mt-1 bg-background">
                        <SelectValue>
                          {(value) => (value === "true" ? t("common:yes") : t("common:no"))}
                        </SelectValue>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="false">{t("common:no")}</SelectItem>
                        <SelectItem value="true">{t("common:yes")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.tags")}
                  </label>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {form.tags.length === 0 && (
                      <span className="text-[11px] text-muted-foreground">
                        {t("templates.noTags")}
                      </span>
                    )}
                    {form.tags.map((tag) => (
                      <button
                        key={tag}
                        type="button"
                        className="rounded-md bg-muted px-2 py-0.5 text-xs hover:bg-destructive/10"
                        onClick={() =>
                          setForm((current) => ({
                            ...current,
                            tags: current.tags.filter((item) => item !== tag),
                          }))
                        }
                      >
                        {tag} ×
                      </button>
                    ))}
                  </div>
                  <div className="mt-2 flex gap-2">
                    <Input
                      value={tagDraft}
                      onChange={(event) => setTagDraft(event.target.value)}
                      placeholder={t("templates.tagsPlaceholder")}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          addListItem("tags", tagDraft, setTagDraft);
                        }
                      }}
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => addListItem("tags", tagDraft, setTagDraft)}
                    >
                      {t("templates.addTag")}
                    </Button>
                  </div>
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("templates.subtasks")}
                  </label>
                  <div className="mt-1 space-y-1">
                    {form.subtasks.length === 0 && (
                      <span className="text-[11px] text-muted-foreground">
                        {t("templates.noSubtasks")}
                      </span>
                    )}
                    {form.subtasks.map((title, index) => (
                      <div key={`${title}-${index}`} className="flex items-center gap-2">
                        <span className="flex-1 truncate text-sm">{title}</span>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          onClick={() =>
                            setForm((current) => ({
                              ...current,
                              subtasks: current.subtasks.filter(
                                (_, itemIndex) => itemIndex !== index,
                              ),
                            }))
                          }
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    ))}
                  </div>
                  <div className="mt-2 flex gap-2">
                    <Input
                      value={subtaskDraft}
                      onChange={(event) => setSubtaskDraft(event.target.value)}
                      placeholder={t("templates.subtasksPlaceholder")}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          addListItem("subtasks", subtaskDraft, setSubtaskDraft);
                        }
                      }}
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => addListItem("subtasks", subtaskDraft, setSubtaskDraft)}
                    >
                      {t("templates.addSubtask")}
                    </Button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        {(error || notice) && (
          <div
            className={`rounded-md px-3 py-2 text-xs ${
              error
                ? "border border-destructive/30 bg-destructive/10 text-destructive"
                : "border border-border bg-muted text-muted-foreground"
            }`}
          >
            {error ?? notice}
          </div>
        )}

        <DialogFooter>
          {mode === "apply" ? (
            <>
              <Button variant="outline" onClick={() => setMode("edit")} disabled={applying}>
                {t("templates.backToEdit")}
              </Button>
              <Button onClick={() => void handleApply()} disabled={applying || !selectedId}>
                {applying ? t("common:loading") : t("templates.submitApply")}
              </Button>
            </>
          ) : (
            <>
              {selectedId && (
                <Button variant="outline" onClick={() => void handleDelete()} disabled={saving}>
                  <Trash2 className="h-4 w-4" />
                  {confirmDelete ? t("templates.confirmDelete") : t("templates.delete")}
                </Button>
              )}
              <Button
                variant="outline"
                onClick={() => {
                  setVariableRows([{}]);
                  setAssigneeId("");
                  setReviewerId("");
                  setError(null);
                  setNotice(null);
                  setMode("apply");
                }}
                disabled={!selectedId}
              >
                <LayoutTemplate className="h-4 w-4" />
                {t("templates.apply")}
              </Button>
              <Button onClick={() => void handleSave()} disabled={saving}>
                {saving ? t("common:loading") : t("templates.save")}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
