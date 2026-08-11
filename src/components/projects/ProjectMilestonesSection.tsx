import { useEffect, useState } from "react";
import { Flag, Pencil, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { Milestone } from "@/lib/types";
import { createMilestone, deleteMilestone, listMilestones, updateMilestone } from "@/lib/backend";
import { formatDate } from "@/lib/utils";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

interface ProjectMilestonesSectionProps {
  projectId: string;
}

export function ProjectMilestonesSection({ projectId }: ProjectMilestonesSectionProps) {
  const { t } = useTranslation(["projects", "common"]);
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [editing, setEditing] = useState<Milestone | null>(null);
  const [name, setName] = useState("");
  const [dueDate, setDueDate] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const reload = async () => {
    setLoading(true);
    setError(null);
    try {
      setMilestones(await listMilestones(projectId));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId]);

  const resetForm = () => {
    setName("");
    setDueDate("");
    setDescription("");
  };

  const openEdit = (milestone: Milestone) => {
    setEditing(milestone);
    setName(milestone.name);
    setDueDate(milestone.due_date ? milestone.due_date.slice(0, 10) : "");
    setDescription(milestone.description ?? "");
  };

  const handleCreate = async () => {
    if (!name.trim()) {
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await createMilestone({
        project_id: projectId,
        name: name.trim(),
        due_date: dueDate || null,
        description: description.trim() || null,
      });
      setCreateOpen(false);
      resetForm();
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleUpdate = async () => {
    if (!editing || !name.trim()) {
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await updateMilestone(editing.id, {
        name: name.trim(),
        due_date: dueDate || null,
        description: description.trim() || null,
      });
      setEditing(null);
      resetForm();
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    setDeletingId(id);
    setError(null);
    try {
      await deleteMilestone(id);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <>
      <Card className="p-4">
        <div className="mb-3 flex items-center justify-between gap-2">
          <h3 className="flex items-center gap-1.5 text-sm font-semibold">
            <Flag className="h-4 w-4 text-primary" />
            {t("milestoneTitle")}
          </h3>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              resetForm();
              setCreateOpen(true);
            }}
          >
            <Plus className="h-3.5 w-3.5" />
            {t("milestoneNew")}
          </Button>
        </div>
        {error && (
          <div className="mb-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        {loading ? (
          <p className="py-4 text-center text-sm text-muted-foreground">{t("common:loading")}</p>
        ) : milestones.length === 0 ? (
          <p className="py-4 text-center text-sm text-muted-foreground">{t("milestoneEmpty")}</p>
        ) : (
          <div className="space-y-2">
            {milestones.map((milestone) => (
              <div
                key={milestone.id}
                className="flex items-start justify-between gap-3 rounded-md border border-border px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">{milestone.name}</p>
                  <p className="mt-0.5 text-[11px] text-muted-foreground">
                    {milestone.due_date
                      ? t("milestoneDue", { date: formatDate(milestone.due_date) })
                      : t("milestoneNoDue")}
                  </p>
                  {milestone.description && (
                    <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                      {milestone.description}
                    </p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-0.5">
                  <button
                    type="button"
                    onClick={() => openEdit(milestone)}
                    className="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
                    title={t("milestoneEditTitle")}
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    disabled={deletingId === milestone.id}
                    onClick={() => void handleDelete(milestone.id)}
                    className="rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
                    title={t("milestoneDeleteTitle")}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("milestoneCreateTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("milestoneNameRequired")}
              </label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="mt-1"
                placeholder={t("milestoneNamePlaceholder")}
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("milestoneDueDate")}
              </label>
              <Input
                type="date"
                value={dueDate}
                onChange={(e) => setDueDate(e.target.value)}
                className="mt-1"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("description")}
              </label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="mt-1 min-h-[72px] resize-y"
                placeholder={t("milestoneOptionalPlaceholder")}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setCreateOpen(false)} disabled={saving}>
                {t("common:cancel")}
              </Button>
              <Button onClick={() => void handleCreate()} disabled={saving || !name.trim()}>
                {saving ? t("creating") : t("create")}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={Boolean(editing)}
        onOpenChange={(open) => {
          if (!open) {
            setEditing(null);
            resetForm();
          }
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("milestoneEditTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("milestoneNameRequired")}
              </label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="mt-1"
                placeholder={t("milestoneNamePlaceholder")}
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("milestoneDueDate")}
              </label>
              <Input
                type="date"
                value={dueDate}
                onChange={(e) => setDueDate(e.target.value)}
                className="mt-1"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                {t("description")}
              </label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="mt-1 min-h-[72px] resize-y"
                placeholder={t("milestoneOptionalPlaceholder")}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  setEditing(null);
                  resetForm();
                }}
                disabled={saving}
              >
                {t("common:cancel")}
              </Button>
              <Button onClick={() => void handleUpdate()} disabled={saving || !name.trim()}>
                {saving ? t("saving") : t("common:save")}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
