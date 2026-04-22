import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Bell,
  Check,
  CheckCheck,
  CircleAlert,
  Info,
  ShieldAlert,
  Siren,
} from "lucide-react";

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
import type {
  NotificationItem,
  NotificationSeverity,
  NotificationType,
} from "@/lib/types";
import { openNotificationTarget } from "@/lib/notificationNavigation";
import { formatDate } from "@/lib/utils";
import { useNotificationStore } from "@/stores/notificationStore";

const severityMeta: Record<NotificationSeverity, {
  label: string;
  icon: typeof Info;
  badgeClassName: string;
  accentClassName: string;
}> = {
  info: {
    label: "信息",
    icon: Info,
    badgeClassName: "border-sky-500/30 bg-sky-500/10 text-sky-700",
    accentClassName: "bg-sky-500",
  },
  success: {
    label: "恢复",
    icon: CheckCheck,
    badgeClassName: "border-emerald-500/30 bg-emerald-500/10 text-emerald-700",
    accentClassName: "bg-emerald-500",
  },
  warning: {
    label: "警告",
    icon: ShieldAlert,
    badgeClassName: "border-amber-500/30 bg-amber-500/10 text-amber-700",
    accentClassName: "bg-amber-500",
  },
  error: {
    label: "错误",
    icon: CircleAlert,
    badgeClassName: "border-orange-500/30 bg-orange-500/10 text-orange-700",
    accentClassName: "bg-orange-500",
  },
  critical: {
    label: "严重",
    icon: Siren,
    badgeClassName: "border-rose-500/30 bg-rose-500/10 text-rose-700",
    accentClassName: "bg-rose-500",
  },
};

const typeLabels: Record<NotificationType, string> = {
  review_pending: "待审核",
  run_failed: "运行失败",
  run_completed: "运行完成",
  task_completed: "任务完成",
  sdk_unavailable: "SDK 异常",
  database_error: "数据库异常",
  ssh_config_error: "SSH 异常",
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

function NotificationRow({
  notification,
  onOpen,
  onMarkRead,
}: NotificationRowProps) {
  const meta = severityMeta[notification.severity];
  const Icon = meta.icon;

  return (
    <div
      className={`rounded-xl border border-border/70 bg-card p-3 transition-colors ${
        notification.is_read ? "opacity-80" : "shadow-sm"
      }`}
    >
      <div className="flex items-start gap-3">
        <div className={`mt-1 h-2.5 w-2.5 shrink-0 rounded-full ${meta.accentClassName}`} />
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-2">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline" className={meta.badgeClassName}>
                  <Icon className="mr-1 h-3.5 w-3.5" />
                  {meta.label}
                </Badge>
                <Badge variant="outline">{typeLabels[notification.notification_type]}</Badge>
                <Badge variant="outline">{notification.source_module}</Badge>
                {notification.delivery_mode === "sticky" && (
                  <Badge variant="outline">持续提醒</Badge>
                )}
                {notification.occurrence_count > 1 && (
                  <Badge variant="outline">x{notification.occurrence_count}</Badge>
                )}
                {notification.is_transient && (
                  <Badge variant="outline">临时</Badge>
                )}
                {!notification.is_read && (
                  <Badge variant="outline">未读</Badge>
                )}
              </div>
              <div>
                <p className="text-sm font-medium leading-5">{notification.title}</p>
                <p className="mt-1 text-sm text-muted-foreground">{notification.message}</p>
                {notification.recommendation && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    建议：{notification.recommendation}
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
              已读
            </Button>
          </div>

          <div className="mt-3 flex items-center justify-between gap-3">
            <div className="text-xs text-muted-foreground">
              触发时间：{formatDate(notification.last_triggered_at)}
            </div>
            {notification.action_route && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void onOpen(notification)}
              >
                {notification.action_label ?? "查看详情"}
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function NotificationCenter() {
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
        aria-label="打开通知中心"
      >
        <Bell className={`h-4 w-4 ${getBellAccent(highestUnreadSeverity)}`} />
        {unreadCount > 0 && (
          <>
            <span className="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-rose-600 px-1 text-[10px] font-medium text-white">
              {unreadCount > 99 ? "99+" : unreadCount}
            </span>
            <span className="sr-only">当前有 {unreadCount} 条未读通知</span>
          </>
        )}
      </Button>

      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent side="right" className="w-full sm:max-w-lg">
          <SheetHeader className="border-b border-border/60 pr-14">
            <div className="flex items-start justify-between gap-3">
              <div>
                <SheetTitle>通知中心</SheetTitle>
                <SheetDescription>
                  {actionableCount > 0
                    ? `当前有 ${actionableCount} 条待处理通知`
                    : "当前没有待处理通知"}
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
                全部已读
              </Button>
            </div>
          </SheetHeader>

          <ScrollArea className="flex-1 px-4 pb-4">
            <div className="space-y-3 pt-4">
              {notifications.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                  当前没有通知。新的审核、运行异常、系统健康问题会主动显示在这里。
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
