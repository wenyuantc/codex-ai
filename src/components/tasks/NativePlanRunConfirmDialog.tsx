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

interface NativePlanRunConfirmDialogProps {
  open: boolean;
  taskTitle: string;
  starting?: boolean;
  onOpenChange: (open: boolean) => void;
  onContinueExisting: () => Promise<void> | void;
  onRegenerate: () => Promise<void> | void;
}

export function NativePlanRunConfirmDialog({
  open,
  taskTitle,
  starting = false,
  onOpenChange,
  onContinueExisting,
  onRegenerate,
}: NativePlanRunConfirmDialogProps) {
  const { t } = useTranslation("tasks");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!starting}>
        <DialogHeader>
          <DialogTitle>{t("nativePlanRunConfirm.title")}</DialogTitle>
          <DialogDescription>
            {t("nativePlanRunConfirm.description", { title: taskTitle })}
          </DialogDescription>
        </DialogHeader>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={starting}
          >
            {t("nativePlanRunConfirm.cancel")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={() => void onRegenerate()}
            disabled={starting}
          >
            {starting ? t("nativePlanRunConfirm.starting") : t("nativePlanRunConfirm.regenerate")}
          </Button>
          <Button type="button" onClick={() => void onContinueExisting()} disabled={starting}>
            {starting
              ? t("nativePlanRunConfirm.starting")
              : t("nativePlanRunConfirm.continueExisting")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
