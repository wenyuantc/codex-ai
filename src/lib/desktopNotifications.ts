import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  onAction,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

import { onDesktopNotificationDeliver } from "@/lib/notifications";
import type { DesktopNotificationEvent, DesktopNotificationExtra } from "@/lib/types";

const transientDeliveryPrefix = "transient:";
const desktopDeliveryReasons = new Set<DesktopNotificationEvent["reason"]>([
  "created",
  "reactivated",
  "updated",
  "transient",
]);

let desktopNotificationRefCount = 0;
let desktopNotificationInitPromise: Promise<void> | null = null;
let desktopNotificationCleanup: (() => void) | null = null;
let desktopNotificationOpenHandler: ((payload: DesktopNotificationExtra) => Promise<void>) | null = null;
let desktopNotificationPermissionRequested = false;

const deliveredDesktopNotificationKeys = new Set<string>();

function releaseDesktopNotificationListeners() {
  desktopNotificationCleanup?.();
  desktopNotificationCleanup = null;
  desktopNotificationInitPromise = null;
}

function isDesktopNotificationExtra(value: unknown): value is DesktopNotificationExtra {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<DesktopNotificationExtra>;
  return (
    typeof candidate.notification_id === "string"
    && typeof candidate.title === "string"
    && typeof candidate.message === "string"
    && typeof candidate.last_triggered_at === "string"
    && typeof candidate.is_transient === "boolean"
    && typeof candidate.reason === "string"
    && desktopDeliveryReasons.has(candidate.reason as DesktopNotificationEvent["reason"])
  );
}

function buildDesktopNotificationKey(event: DesktopNotificationEvent) {
  if (event.is_transient || event.notification_id.startsWith(transientDeliveryPrefix)) {
    return `${transientDeliveryPrefix}${event.notification_id}`;
  }

  return `${event.reason}:${event.notification_id}:${event.last_triggered_at}`;
}

async function shouldSendDesktopNotification() {
  try {
    const window = getCurrentWindow();
    const [visible, focused] = await Promise.all([
      window.isVisible(),
      window.isFocused(),
    ]);
    return !(visible && focused);
  } catch (error) {
    console.error("Failed to inspect window visibility for desktop notifications:", error);
    return true;
  }
}

async function ensureDesktopNotificationPermission() {
  try {
    if (await isPermissionGranted()) {
      return true;
    }

    if (desktopNotificationPermissionRequested) {
      return false;
    }

    desktopNotificationPermissionRequested = true;
    const permission = await requestPermission();
    return permission === "granted";
  } catch (error) {
    console.error("Failed to request desktop notification permission:", error);
    return false;
  }
}

async function deliverDesktopNotification(event: DesktopNotificationEvent) {
  const deliveryKey = buildDesktopNotificationKey(event);
  if (deliveredDesktopNotificationKeys.has(deliveryKey)) {
    return;
  }

  deliveredDesktopNotificationKeys.add(deliveryKey);

  if (!(await shouldSendDesktopNotification())) {
    return;
  }

  if (!(await ensureDesktopNotificationPermission())) {
    return;
  }

  sendNotification({
    title: event.title,
    body: event.message,
    autoCancel: true,
    extra: event as unknown as Record<string, unknown>,
  });
}

async function handleDesktopNotificationAction(payload: { extra?: Record<string, unknown> }) {
  if (!isDesktopNotificationExtra(payload.extra)) {
    return;
  }

  await desktopNotificationOpenHandler?.(payload.extra);
}

export function initDesktopNotificationBridge(
  onOpen: (payload: DesktopNotificationExtra) => Promise<void>,
) {
  desktopNotificationOpenHandler = onOpen;
  desktopNotificationRefCount += 1;

  if (!desktopNotificationInitPromise && !desktopNotificationCleanup) {
    // Desktop delivery is reliable, but click callbacks vary by platform/plugin support.
    desktopNotificationInitPromise = Promise.all([
      onDesktopNotificationDeliver((payload) => {
        void deliverDesktopNotification(payload);
      }),
      onAction((payload) => {
        void handleDesktopNotificationAction(payload);
      }),
    ])
      .then(([notificationUnlisten, actionListener]) => {
        desktopNotificationCleanup = () => {
          void notificationUnlisten();
          void actionListener.unregister();
        };
        desktopNotificationInitPromise = null;

        if (desktopNotificationRefCount === 0) {
          releaseDesktopNotificationListeners();
        }
      })
      .catch((error) => {
        console.error("Failed to initialize desktop notification bridge:", error);
        desktopNotificationInitPromise = null;
        desktopNotificationCleanup = null;
      });
  }

  let released = false;

  return () => {
    if (released) {
      return;
    }

    released = true;
    desktopNotificationRefCount = Math.max(0, desktopNotificationRefCount - 1);

    if (desktopNotificationRefCount === 0) {
      desktopNotificationOpenHandler = null;
      if (desktopNotificationCleanup) {
        releaseDesktopNotificationListeners();
      }
    }
  };
}
