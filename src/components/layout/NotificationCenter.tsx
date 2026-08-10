import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Bell, Check, CheckCheck, CircleAlert, Info, ShieldAlert, Siren } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import type { NotificationItem, NotificationSeverity, NotificationType } from "@/lib/types";
import { openNotificationTarget } from "@/lib/notificationNavigation";
import { formatDate } from "@/lib/utils";
import { useNotificationStore } from "@/stores/notificationStore";

const severityVisual: Record<
  NotificationSeverity,
  {
    icon: typeof Info;
    badgeClassName: string;
    accentClassName: string;
  }
> = {
  info: {
    icon: Info,
    badgeClassName: "border-sky-500/30 bg-sky-500/10 text-sky-700",
    accentClassName: "bg-sky-500",
  },
  success: {
    icon: CheckCheck,
    badgeClassName: "border-emerald-500/30 bg-emerald-500/10 text-emerald-700",
    accentClassName: "bg-emerald-500",
  },
  warning: {
    icon: ShieldAlert,
    badgeClassName: "border-amber-500/30 bg-amber-500/10 text-amber-700",
    accentClassName: "bg-amber-500",
  },
  error: {
    icon: CircleAlert,
    badgeClassName: "border-orange-500/30 bg-orange-500/10 text-orange-700",
    accentClassName: "bg-orange-500",
  },
  critical: {
    icon: Siren,
    badgeClassName: "border-rose-500/30 bg-rose-500/10 text-rose-700",
    accentClassName: "bg-rose-500",
  },
};

function getBellAccent(severity: NotificationSeverity | null) {
  if (severity === "critical") {
    return "text-rose-600";
  }
  if (severity === "error") {
    return "text-orange-600";
  }
  if (severity === "warning") {
    return "text-amber-600";
  }
  return "text-foreground";
}

interface NotificationRowProps {
  notification: NotificationItem;
  onOpen: (notification: NotificationItem) => Promise<void>;
  onMarkRead: (id: string) => Promise<void>;
}

function NotificationRow({ notification, onOpen, onMarkRead }: NotificationRowProps) {
  const { t } = useTranslation("notifications");
  const visual = severityVisual[notification.severity];
  const Icon = visual.icon;
  const severityLabel = t(`severity.${notification.severity}`);
  const typeLabel = t(`types.${notification.notification_type as NotificationType}`);

  return (
    <div
      className={`rounded-xl border border-border/70 bg-card p-3 transition-colors ${
        notification.is_read ? "opacity-80" : "shadow-sm"
      }`}
    >
      <div className="flex items-start gap-3">
        <div className={`mt-1 h-2.5 w-2.5 shrink-0 rounded-full ${visual.accentClassName}`} />
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-2">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline" className={visual.badgeClassName}>
                  <Icon className="mr-1 h-3.5 w-3.5" />
                  {severityLabel}
                </Badge>
                <Badge variant="outline">{typeLabel}</Badge>
                <Badge variant="outline">{notification.source_module}</Badge>
                {notification.delivery_mode === "sticky" && (
                  <Badge variant="outline">{t("sticky")}</Badge>
                )}
                {notification.occurrence_count > 1 && (
                  <Badge variant="outline">x{notification.occurrence_count}</Badge>
                )}
                {notification.is_transient && <Badge variant="outline">{t("transient")}</Badge>}
                {!notification.is_read && <Badge variant="outline">{t("unread")}</Badge>}
              </div>
              <div>
                <p className="text-sm font-medium leading-5">{notification.title}</p>
                <p className="mt-1 text-sm text-muted-foreground">{notification.message}</p>
                {notification.recommendation && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    {t("recommendation", { text: notification.recommendation })}
                  </p>
                )}
              </div>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={notification.is_read}
              onClick={() => void onMarkRead(notification.id)}
            >
              <Check className="h-4 w-4" />
              {t("markRead")}
            </Button>
          </div>

          <div className="mt-3 flex items-center justify-between gap-3">
            <div className="text-xs text-muted-foreground">
              {t("triggeredAt", { time: formatDate(notification.last_triggered_at) })}
            </div>
            {notification.action_route && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void onOpen(notification)}
              >
                {notification.action_label ?? t("viewDetails")}
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function NotificationCenter() {
  const { t } = useTranslation("notifications");
  const navigate = useNavigate();
  const notifications = useNotificationStore((state) => state.notifications);
  const unreadCount = useNotificationStore((state) => state.unreadCount);
  const highestUnreadSeverity = useNotificationStore((state) => state.highestUnreadSeverity);
  const markRead = useNotificationStore((state) => state.markRead);
  const markAllRead = useNotificationStore((state) => state.markAllRead);
  const [open, setOpen] = useState(false);

  const actionableCount = useMemo(
    () => notifications.filter((notification) => !notification.is_read).length,
    [notifications],
  );

  const handleOpenNotification = async (notification: NotificationItem) => {
    await markRead(notification.id);
    setOpen(false);
    await openNotificationTarget(navigate, notification);
  };

  return (
    <>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="relative"
        onClick={() => setOpen(true)}
        aria-label={t("openAria")}
      >
        <Bell className={`h-4 w-4 ${getBellAccent(highestUnreadSeverity)}`} />
        {unreadCount > 0 && (
          <>
            <span className="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-rose-600 px-1 text-[10px] font-medium text-white">
              {unreadCount > 99 ? "99+" : unreadCount}
            </span>
            <span className="sr-only">{t("unreadSr", { count: unreadCount })}</span>
          </>
        )}
      </Button>

      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent side="right" className="w-full sm:max-w-lg">
          <SheetHeader className="border-b border-border/60 pr-14">
            <div className="flex items-start justify-between gap-3">
              <div>
                <SheetTitle>{t("title")}</SheetTitle>
                <SheetDescription>
                  {actionableCount > 0
                    ? t("pendingDescription", { count: actionableCount })
                    : t("noPendingDescription")}
                </SheetDescription>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                disabled={unreadCount === 0}
                onClick={() => void markAllRead()}
              >
                <CheckCheck className="h-4 w-4" />
                {t("markAllRead")}
              </Button>
            </div>
          </SheetHeader>

          <ScrollArea className="flex-1 px-4 pb-4">
            <div className="space-y-3 pt-4">
              {notifications.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                  {t("emptyDetail")}
                </div>
              ) : (
                notifications.map((notification) => (
                  <NotificationRow
                    key={notification.id}
                    notification={notification}
                    onOpen={handleOpenNotification}
                    onMarkRead={markRead}
                  />
                ))
              )}
            </div>
          </ScrollArea>
        </SheetContent>
      </Sheet>
    </>
  );
}
