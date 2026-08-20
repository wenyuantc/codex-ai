import { useEffect, useState } from "react";
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
import {
  completeImageSkipConfirm,
  getPendingImageSkipConfirm,
  subscribeImageSkipConfirm,
} from "@/lib/imageAttachmentSkip";

export function ImageSkipConfirmDialog() {
  const { t } = useTranslation("tasks");
  const [tick, setTick] = useState(0);

  useEffect(() => subscribeImageSkipConfirm(() => setTick((value) => value + 1)), []);

  const pending = getPendingImageSkipConfirm();
  const open = Boolean(pending);
  void tick;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && getPendingImageSkipConfirm()) {
          completeImageSkipConfirm(false);
        }
      }}
    >
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t("imageSkip.title")}</DialogTitle>
          <DialogDescription>
            {pending ? t(`imageSkip.${pending.reason}`, { count: pending.count }) : null}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="mt-2">
          <Button type="button" variant="outline" onClick={() => completeImageSkipConfirm(false)}>
            {t("imageSkip.cancel")}
          </Button>
          <Button type="button" onClick={() => completeImageSkipConfirm(true)}>
            {t("imageSkip.continue")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
