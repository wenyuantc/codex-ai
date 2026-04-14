import { useState } from "react";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
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
  const { projects } = useProjectStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("medium");
  const [selectedProjectId, setSelectedProjectId] = useState(
    projectId ?? ""
  );
  const [assigneeId, setAssigneeId] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const handleOpen = (isOpen: boolean) => {
    if (isOpen) {
      fetchEmployees();
      setTitle("");
      setDescription("");
      setPriority("medium");
      setSelectedProjectId(projectId ?? "");
      setAssigneeId("");
      setCreateError(null);
    }
    onOpenChange(isOpen);
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
      <DialogContent className="max-w-md">
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

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              描述
            </label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="任务描述（可选）"
              className="mt-1 min-h-[60px] resize-y"
            />
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
