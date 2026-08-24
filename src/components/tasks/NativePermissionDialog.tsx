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
  onNativePermissionRequest,
  resolveNativeToolPermission,
  type NativePermissionRequest,
} from "@/lib/native";

export function NativePermissionDialog() {
  const { t } = useTranslation("tasks");
  const [pending, setPending] = useState<NativePermissionRequest | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onNativePermissionRequest((request) => {
      setPending(request);
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const resolve = (decision: "allow_session" | "allow_once" | "deny") => {
    if (!pending) {
      return;
    }
    const current = pending;
    setPending(null);
    void resolveNativeToolPermission(current.sessionRecordId, current.requestId, decision);
  };

  return (
    <Dialog
      open={Boolean(pending)}
      onOpenChange={(next) => {
        if (!next && pending) {
          resolve("deny");
        }
      }}
    >
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t("nativePermission.title")}</DialogTitle>
          <DialogDescription>
            {pending
              ? t("nativePermission.summary", {
                  location: pending.remote
                    ? t("nativePermission.remote")
                    : t("nativePermission.local"),
                  kind: t(`nativePermission.kind.${pending.kind}`),
                  summary: pending.summary,
                  tool: pending.toolName,
                })
              : null}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="mt-2 flex-col gap-2 sm:flex-col">
          <Button type="button" onClick={() => resolve("allow_session")}>
            {t("nativePermission.allowSession")}
          </Button>
          <Button type="button" variant="outline" onClick={() => resolve("allow_once")}>
            {t("nativePermission.allowOnce")}
          </Button>
          <Button type="button" variant="ghost" onClick={() => resolve("deny")}>
            {t("nativePermission.deny")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
