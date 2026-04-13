import { useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { RepoPathField } from "@/components/projects/RepoPathField";

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const { createProject } = useProjectStore();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [repoPath, setRepoPath] = useState("");
  const [saving, setSaving] = useState(false);

  const handleOpen = (isOpen: boolean) => {
    if (isOpen) {
      setName("");
      setDescription("");
      setRepoPath("");
    }
    onOpenChange(isOpen);
  };

  const handleCreate = async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      await createProject({
        name: name.trim(),
        description: description.trim() || undefined,
        repo_path: repoPath.trim() || undefined,
      });
      handleOpen(false);
    } finally {
      setSaving(false);
    }
  };

  const handleRepoPathSelected = (path: string) => {
    if (name.trim()) return;

    const normalized = path.replace(/[\\/]+$/, "");
    const directoryName = normalized.split(/[\\/]/).filter(Boolean).pop();

    if (directoryName) {
      setName(directoryName);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpen}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>新建项目</DialogTitle>
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

          <RepoPathField
            value={repoPath}
            onChange={setRepoPath}
            onDirectorySelected={handleRepoPathSelected}
          />

          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={() => handleOpen(false)}
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
