import { useTranslation } from "react-i18next";
import type { Employee } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface DeleteEmployeeDialogProps {
  open: boolean;
  employee: Employee | null;
  deleting?: boolean;
  error?: string | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<void> | void;
}

export function DeleteEmployeeDialog({
  open,
  employee,
  deleting = false,
  error = null,
  onOpenChange,
  onConfirm,
}: DeleteEmployeeDialogProps) {
  const { t } = useTranslation(["employees", "common"]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md" showCloseButton={!deleting}>
        <DialogHeader>
          <DialogTitle>{t("deleteTitle")}</DialogTitle>
          <DialogDescription>
            {t("deleteDescription", { name: employee?.name ?? "" })}
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <p className="text-sm text-destructive">{t("deleteFailed", { message: error })}</p>
        ) : null}

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
