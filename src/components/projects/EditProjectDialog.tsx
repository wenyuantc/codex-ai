import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useProjectStore } from "@/stores/projectStore";
import type { Project, ProjectType } from "@/lib/types";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
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
import { getProjectTypeLabel } from "@/lib/projects";
import { getStatusLabel } from "@/lib/utils";

interface EditProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  project: Project | null;
}

export function EditProjectDialog({ open, onOpenChange, project }: EditProjectDialogProps) {
  const { t } = useTranslation(["projects", "common"]);
  const { updateProject, sshConfigs, fetchSshConfigs } = useProjectStore();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [projectType, setProjectType] = useState<ProjectType>("local");
  const [repoPath, setRepoPath] = useState("");
  const [sshConfigId, setSshConfigId] = useState("");
  const [remoteRepoPath, setRemoteRepoPath] = useState("");
  const [testCommand, setTestCommand] = useState("");
  const [status, setStatus] = useState("active");
  const [saving, setSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (open && project) {
      void fetchSshConfigs();
      setName(project.name);
      setDescription(project.description ?? "");
      setProjectType(project.project_type);
      setRepoPath(project.repo_path ?? "");
      setSshConfigId(project.ssh_config_id ?? "");
      setRemoteRepoPath(project.remote_repo_path ?? "");
      setTestCommand(project.test_command ?? "");
      setStatus(project.status);
      setErrorMessage(null);
    }
  }, [fetchSshConfigs, open, project]);

  const handleSave = async () => {
    if (!name.trim() || !project) return;
    if (projectType === "local" && !repoPath.trim()) {
      setErrorMessage(t("errorLocalPathRequired"));
      return;
    }
    if (projectType === "ssh" && (!sshConfigId || !remoteRepoPath.trim())) {
      setErrorMessage(t("errorSshRequired"));
      return;
    }

    setSaving(true);
    setErrorMessage(null);
    try {
      await updateProject(project.id, {
        name: name.trim(),
        description: description.trim() || null,
        project_type: projectType,
        repo_path: projectType === "local" ? repoPath.trim() || null : null,
        ssh_config_id: projectType === "ssh" ? sshConfigId : null,
        remote_repo_path: projectType === "ssh" ? remoteRepoPath.trim() || null : null,
        test_command: testCommand.trim() || null,
        status,
      });
      onOpenChange(false);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t("editTitle")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("projectName")} *
            </label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("projectName")}
              className="mt-1"
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">{t("description")}</label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("descriptionOptional")}
              className="mt-1 min-h-[60px] resize-y"
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("projectType")} *
            </label>
            <Select
              value={projectType}
              onValueChange={(value) => {
                setProjectType(value === "ssh" ? "ssh" : "local");
                setErrorMessage(null);
              }}
            >
              <SelectTrigger className="mt-1">
                <SelectValue>
                  {(value) =>
                    getProjectTypeLabel(
                      typeof value === "string" ? (value as ProjectType) : projectType,
                    )
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="local">{t("common:projectType.local")}</SelectItem>
                <SelectItem value="ssh">{t("common:projectType.ssh")}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {projectType === "local" ? (
            <RepoPathField value={repoPath} onChange={setRepoPath} />
          ) : (
            <>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("sshConfig")} *
                </label>
                <Select
                  value={sshConfigId || null}
                  onValueChange={(value) => {
                    setSshConfigId(value ?? "");
                    setErrorMessage(null);
                  }}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue placeholder={t("selectSshConfig")}>
                      {(value) =>
                        sshConfigs.find((config) => config.id === value)?.name ??
                        t("selectSshConfig")
                      }
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
                <label className="text-xs font-medium text-muted-foreground">
                  {t("remoteRepoDir")} *
                </label>
                <Input
                  value={remoteRepoPath}
                  onChange={(e) => setRemoteRepoPath(e.target.value)}
                  placeholder="/srv/repos/my-project"
                  className="mt-1"
                />
              </div>
            </>
          )}

          <div>
            <label className="text-xs font-medium text-muted-foreground">{t("testCommand")}</label>
            <Input
              value={testCommand}
              onChange={(e) => setTestCommand(e.target.value)}
              placeholder={t("testCommandPlaceholder")}
              className="mt-1 font-mono text-sm"
            />
            <p className="mt-1 text-[11px] text-muted-foreground">{t("testCommandHint")}</p>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">{t("status")}</label>
            <Select value={status} onValueChange={(value) => setStatus(value ?? "active")}>
              <SelectTrigger className="mt-1">
                <SelectValue placeholder={t("selectStatus")}>
                  {(value) =>
                    typeof value === "string" ? getStatusLabel(value) : t("selectStatus")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="active">{t("common:status.active")}</SelectItem>
                <SelectItem value="archived">{t("common:status.archived")}</SelectItem>
              </SelectContent>
            </Select>
          </div>

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
              {t("common:cancel")}
            </button>
            <button
              onClick={handleSave}
              disabled={
                saving ||
                !name.trim() ||
                (projectType === "local"
                  ? !repoPath.trim()
                  : !sshConfigId || !remoteRepoPath.trim())
              }
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? t("saving") : t("common:save")}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
