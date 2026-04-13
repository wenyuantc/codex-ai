import { useState, useEffect } from "react";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/lib/types";
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
import { Textarea } from "@/components/ui/textarea";
import { RepoPathField } from "@/components/projects/RepoPathField";
import { getStatusLabel } from "@/lib/utils";

interface EditProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  project: Project | null;
}

export function EditProjectDialog({ open, onOpenChange, project }: EditProjectDialogProps) {
  const { updateProject } = useProjectStore();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [repoPath, setRepoPath] = useState("");
  const [status, setStatus] = useState("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open && project) {
      setName(project.name);
      setDescription(project.description ?? "");
      setRepoPath(project.repo_path ?? "");
      setStatus(project.status);
    }
  }, [open, project]);

  const handleSave = async () => {
    if (!name.trim() || !project) return;
    setSaving(true);
    try {
      await updateProject(project.id, {
        name: name.trim(),
        description: description.trim() || null,
        repo_path: repoPath.trim() || null,
        status,
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
          <DialogTitle>编辑项目</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">项目名称 *</label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="项目名称"
              className="mt-1"
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">描述</label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="项目描述（可选）"
              className="mt-1 min-h-[60px] resize-y"
            />
          </div>

          <RepoPathField value={repoPath} onChange={setRepoPath} />

          <div>
            <label className="text-xs font-medium text-muted-foreground">状态</label>
            <Select
              value={status}
              onValueChange={(value) => setStatus(value ?? "active")}
            >
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="选择状态">
                  {(value) => (typeof value === "string" ? getStatusLabel(value) : "选择状态")}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="active">活跃</SelectItem>
                <SelectItem value="archived">归档</SelectItem>
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
              disabled={!name.trim() || saving}
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
