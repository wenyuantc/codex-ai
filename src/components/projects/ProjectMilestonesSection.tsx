import { useEffect, useState } from "react";
import { Flag, Plus, Trash2 } from "lucide-react";

import type { Milestone } from "@/lib/types";
import {
  createMilestone,
  deleteMilestone,
  listMilestones,
} from "@/lib/backend";
import { formatDate } from "@/lib/utils";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ProjectMilestonesSectionProps {
  projectId: string;
}

export function ProjectMilestonesSection({ projectId }: ProjectMilestonesSectionProps) {
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
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
      setName("");
      setDueDate("");
      setDescription("");
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
            里程碑
          </h3>
          <Button size="sm" variant="outline" onClick={() => setCreateOpen(true)}>
            <Plus className="h-3.5 w-3.5" />
            新建
          </Button>
        </div>
        {error && (
          <div className="mb-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        {loading ? (
          <p className="py-4 text-center text-sm text-muted-foreground">加载中…</p>
        ) : milestones.length === 0 ? (
          <p className="py-4 text-center text-sm text-muted-foreground">暂无里程碑</p>
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
                      ? `截止：${formatDate(milestone.due_date)}`
                      : "未设置截止日期"}
                  </p>
                  {milestone.description && (
                    <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                      {milestone.description}
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  disabled={deletingId === milestone.id}
                  onClick={() => void handleDelete(milestone.id)}
                  className="shrink-0 rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
                  title="删除里程碑"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>新建里程碑</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">名称 *</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="mt-1"
                placeholder="例如：MVP 发布"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">截止日期</label>
              <Input
                type="date"
                value={dueDate}
                onChange={(e) => setDueDate(e.target.value)}
                className="mt-1"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">描述</label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="mt-1 min-h-[72px] resize-y"
                placeholder="可选"
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setCreateOpen(false)} disabled={saving}>
                取消
              </Button>
              <Button onClick={() => void handleCreate()} disabled={saving || !name.trim()}>
                {saving ? "创建中…" : "创建"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
