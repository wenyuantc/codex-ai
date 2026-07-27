import { useEffect, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import type { ProjectType } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { RepoPathField } from "@/components/projects/RepoPathField";
import { getProjectTypeLabel } from "@/lib/projects";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { createMilestone, createTag } from "@/lib/backend";

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const { createProject, sshConfigs, fetchSshConfigs } = useProjectStore();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [projectType, setProjectType] = useState<ProjectType>("local");
  const [repoPath, setRepoPath] = useState("");
  const [sshConfigId, setSshConfigId] = useState("");
  const [remoteRepoPath, setRemoteRepoPath] = useState("");
  const [applyScaffold, setApplyScaffold] = useState(true);
  const [saving, setSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const resetForm = () => {
    setName("");
    setDescription("");
    setProjectType("local");
    setRepoPath("");
    setSshConfigId("");
    setRemoteRepoPath("");
    setApplyScaffold(true);
    setErrorMessage(null);
  };

  useEffect(() => {
    if (open) {
      resetForm();
      void fetchSshConfigs();
    }
  }, [fetchSshConfigs, open]);

  const handleCreate = async () => {
    if (!name.trim()) return;
    if (projectType === "local" && !repoPath.trim()) {
      setErrorMessage("本地项目必须填写本地仓库路径。");
      return;
    }
    if (projectType === "ssh" && (!sshConfigId || !remoteRepoPath.trim())) {
      setErrorMessage("SSH 项目必须选择 SSH 配置并填写远程仓库目录。");
      return;
    }

    setSaving(true);
    setErrorMessage(null);
    try {
      const project = await createProject({
        name: name.trim(),
        description: description.trim() || undefined,
        project_type: projectType,
        repo_path: projectType === "local" ? repoPath.trim() || undefined : null,
        ssh_config_id: projectType === "ssh" ? sshConfigId : null,
        remote_repo_path: projectType === "ssh" ? remoteRepoPath.trim() || undefined : null,
      });

      if (applyScaffold && project?.id) {
        await Promise.allSettled([
          createTag({ project_id: project.id, name: "bug", color: "#ef4444" }),
          createTag({ project_id: project.id, name: "feature", color: "#3b82f6" }),
          createTag({ project_id: project.id, name: "chore", color: "#64748b" }),
          createMilestone({
            project_id: project.id,
            name: "MVP",
            description: "项目脚手架自动创建的首个里程碑",
          }),
        ]);
      }

      resetForm();
      onOpenChange(false);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
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

  const canSubmit = projectType === "local"
    ? Boolean(name.trim() && repoPath.trim())
    : Boolean(name.trim() && sshConfigId && remoteRepoPath.trim());

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
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

          <div>
            <label className="text-xs font-medium text-muted-foreground">项目类型 *</label>
            <Select
              value={projectType}
              onValueChange={(value) => {
                const nextType = value === "ssh" ? "ssh" : "local";
                setProjectType(nextType);
                setErrorMessage(null);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => getProjectTypeLabel(typeof value === "string" ? value as ProjectType : projectType)}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="local">本地项目</SelectItem>
                <SelectItem value="ssh">SSH 项目</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {projectType === "local" ? (
            <RepoPathField
              value={repoPath}
              onChange={setRepoPath}
              onDirectorySelected={handleRepoPathSelected}
            />
          ) : (
            <>
              <div>
                <label className="text-xs font-medium text-muted-foreground">SSH 配置 *</label>
                <Select
                  value={sshConfigId || null}
                  onValueChange={(value) => {
                    setSshConfigId(value ?? "");
                    setErrorMessage(null);
                  }}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder="选择 SSH 配置">
                      {(value) => sshConfigs.find((config) => config.id === value)?.name ?? "选择 SSH 配置"}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {sshConfigs.map((config) => (
                      <SelectItem key={config.id} value={config.id}>
                        {config.name} ({config.username}@{config.host})
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div>
                <label className="text-xs font-medium text-muted-foreground">远程仓库目录 *</label>
                <Input
                  value={remoteRepoPath}
                  onChange={(e) => setRemoteRepoPath(e.target.value)}
                  placeholder="/srv/repos/my-project"
                  className="mt-1"
                />
              </div>
            </>
          )}

          <label className="flex items-start gap-2 rounded-md border border-border px-3 py-2 text-xs">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={applyScaffold}
              onChange={(e) => setApplyScaffold(e.target.checked)}
            />
            <span>
              <span className="font-medium">应用项目脚手架</span>
              <span className="mt-0.5 block text-muted-foreground">
                自动创建标签（bug/feature/chore）与首个里程碑 MVP，便于交付管理开箱即用。
              </span>
            </span>
          </label>

          {errorMessage && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {errorMessage}
            </div>
          )}

          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={() => onOpenChange(false)}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent"
            >
              取消
            </button>
            <button
              onClick={handleCreate}
              disabled={!canSubmit || saving}
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
