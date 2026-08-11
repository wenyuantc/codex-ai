import { Button } from "@/components/ui/button";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface ConfirmPermanentDeleteDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  deleting?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}

export function ConfirmPermanentDeleteDialog({
  open,
  title,
  description,
  confirmLabel,
  deleting = false,
  onOpenChange,
  onConfirm,
}: ConfirmPermanentDeleteDialogProps) {
  const { t } = useTranslation(["trash", "common"]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!deleting}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>

        <DialogFooter className="mt-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={deleting}
          >
            {t("common:cancel")}
          </Button>
          <Button type="button" variant="destructive" onClick={onConfirm} disabled={deleting}>
            {deleting ? t("deleting") : (confirmLabel ?? t("permanentDelete"))}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
