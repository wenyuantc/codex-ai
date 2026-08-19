import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { createTaskTemplateFromTask } from "@/lib/backend";
import type { Task } from "@/lib/types";

interface SaveTaskAsTemplateDialogProps {
  open: boolean;
  task: Task;
  onOpenChange: (open: boolean) => void;
}

export function SaveTaskAsTemplateDialog({
  open,
  task,
  onOpenChange,
}: SaveTaskAsTemplateDialogProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const [name, setName] = useState(task.title);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setName(task.title);
    setSaving(false);
    setError(null);
  }, [open, task.id, task.title]);

  const handleSave = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError(t("templates.needNameAndTitle"));
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await createTaskTemplateFromTask(task.id, trimmed);
      onOpenChange(false);
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : String(saveError));
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("templates.saveAsTitle")}</DialogTitle>
        </DialogHeader>
        <p className="text-xs text-muted-foreground">{t("templates.saveAsHint")}</p>
        <div>
          <label className="text-xs font-medium text-muted-foreground">
            {t("templates.saveAsName")}
          </label>
          <Input
            className="mt-1"
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={saving}
          />
        </div>
        {error && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>
            {t("common:cancel")}
          </Button>
          <Button onClick={() => void handleSave()} disabled={saving}>
            {saving ? t("common:loading") : t("templates.saveAsSubmit")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
