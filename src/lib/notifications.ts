import { listen } from "@tauri-apps/api/event";

import type { NotificationCenterChanged, TransientNotification } from "./types";

const TRANSIENT_NOTIFICATION_ID_PREFIXES = ["transient:", "transient-"];

export function isTransientNotificationId(id: string) {
  return TRANSIENT_NOTIFICATION_ID_PREFIXES.some((prefix) => id.startsWith(prefix));
}

export function onNotificationCenterChanged(
  callback: (payload: NotificationCenterChanged) => void,
) {
  return listen<NotificationCenterChanged>(
    "notification-center-changed",
    (event) => callback(event.payload),
  );
}

export function onTransientNotification(
  callback: (payload: TransientNotification) => void,
) {
  return listen<TransientNotification>(
    "notification-center-transient",
    (event) => callback(event.payload),
  );
}
