import type { TaskGitContext } from "@/lib/types";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface DeleteTaskGitContextDialogProps {
  open: boolean;
  context: TaskGitContext | null;
  deleting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void> | void;
}

export function DeleteTaskGitContextDialog({
  open,
  context,
  deleting = false,
  onOpenChange,
  onConfirm,
}: DeleteTaskGitContextDialogProps) {
  const { t } = useTranslation(["projects", "common"]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!deleting}>
        <DialogHeader>
          <DialogTitle>{t("deleteGitContextTitle")}</DialogTitle>
          <DialogDescription>
            {t("deleteGitContextDescription", {
              branch: context?.task_branch ?? t("deleteGitContextUnnamedBranch"),
            })}
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <div>{t("targetBranch", { branch: context?.target_branch ?? t("notSet") })}</div>
          <div className="mt-1">{t("worktreeMissingHint")}</div>
        </div>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={deleting}
          >
            {t("common:cancel")}
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={() => void onConfirm()}
            disabled={deleting}
          >
            {deleting ? t("deleting") : t("confirmDelete")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
