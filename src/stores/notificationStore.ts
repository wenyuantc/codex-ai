import { create } from "zustand";

import {
  listNotifications as listNotificationsCommand,
  markAllNotificationsRead as markAllNotificationsReadCommand,
  markNotificationRead as markNotificationReadCommand,
  syncSystemNotifications as syncSystemNotificationsCommand,
} from "@/lib/backend";
import {
  isTransientNotificationId,
  onNotificationCenterChanged,
  onTransientNotification,
} from "@/lib/notifications";
import type {
  AppNotification,
  EnvironmentMode,
  NotificationItem,
  NotificationSeverity,
  TransientNotification,
} from "@/lib/types";

function sortNotifications(items: NotificationItem[]) {
  return [...items].sort((left, right) => {
    const timeDelta = right.last_triggered_at.localeCompare(left.last_triggered_at);
    if (timeDelta !== 0) {
      return timeDelta;
    }

    return right.id.localeCompare(left.id);
  });
}

function mergeNotifications(
  persisted: AppNotification[],
  transient: TransientNotification[],
) {
  return sortNotifications([...transient, ...persisted]);
}

function getHighestUnreadSeverity(
  notifications: NotificationItem[],
): NotificationSeverity | null {
  const severityRank: Record<NotificationSeverity, number> = {
    info: 1,
    success: 2,
    warning: 3,
    error: 4,
    critical: 5,
  };

  return notifications.reduce<NotificationSeverity | null>((current, notification) => {
    if (notification.is_read) {
      return current;
    }

    if (!current || severityRank[notification.severity] > severityRank[current]) {
      return notification.severity;
    }

    return current;
  }, null);
}

function buildDerivedState(
  persisted: AppNotification[],
  transient: TransientNotification[],
) {
  const notifications = mergeNotifications(persisted, transient);
  const unreadCount = notifications.filter((notification) => !notification.is_read).length;
  return {
    persistedNotifications: persisted,
    transientNotifications: transient,
    notifications,
    unreadCount,
    highestUnreadSeverity: getHighestUnreadSeverity(notifications),
  };
}

interface NotificationStore {
  persistedNotifications: AppNotification[];
  transientNotifications: TransientNotification[];
  notifications: NotificationItem[];
  unreadCount: number;
  highestUnreadSeverity: NotificationSeverity | null;
  loading: boolean;
  fetchNotifications: (limit?: number) => Promise<void>;
  markRead: (id: string) => Promise<void>;
  markAllRead: () => Promise<void>;
  syncSystemNotifications: (
    environmentMode: EnvironmentMode,
    selectedSshConfigId?: string | null,
  ) => Promise<void>;
  initNotificationListeners: () => () => void;
}

let notificationListenerRefCount = 0;
let notificationListenersInitPromise: Promise<void> | null = null;
let notificationListenersCleanup: (() => void) | null = null;

function releaseNotificationListeners() {
  notificationListenersCleanup?.();
  notificationListenersCleanup = null;
  notificationListenersInitPromise = null;
}

export const useNotificationStore = create<NotificationStore>((set, get) => ({
  persistedNotifications: [],
  transientNotifications: [],
  notifications: [],
  unreadCount: 0,
  highestUnreadSeverity: null,
  loading: false,

  fetchNotifications: async (limit = 60) => {
    set({ loading: true });
    try {
      const notifications = await listNotificationsCommand(limit);
      set(() => ({
        ...buildDerivedState(notifications, []),
        loading: false,
      }));
    } catch (error) {
      console.error("Failed to fetch notifications:", error);
      set({ loading: false });
    }
  },

  markRead: async (id) => {
    const isTransientNotification = get().transientNotifications.some(
      (notification) => notification.id === id,
    ) || isTransientNotificationId(id);

    if (isTransientNotification) {
      set((state) => {
        const transientNotifications = state.transientNotifications.map((notification) => (
          notification.id === id
            ? { ...notification, is_read: true }
            : notification
        ));
        return buildDerivedState(state.persistedNotifications, transientNotifications);
      });
      return;
    }

    try {
      await markNotificationReadCommand(id);
      await get().fetchNotifications();
    } catch (error) {
      console.error(`Failed to mark notification ${id} as read:`, error);
    }
  },

  markAllRead: async () => {
    set((state) => {
      const transientNotifications = state.transientNotifications.map((notification) => ({
        ...notification,
        is_read: true,
      }));
      return buildDerivedState(state.persistedNotifications, transientNotifications);
    });

    try {
      await markAllNotificationsReadCommand();
      await get().fetchNotifications();
    } catch (error) {
      console.error("Failed to mark all notifications as read:", error);
    }
  },

  syncSystemNotifications: async (environmentMode, selectedSshConfigId) => {
    try {
      await syncSystemNotificationsCommand(environmentMode, selectedSshConfigId);
    } catch (error) {
      console.error("Failed to sync system notifications:", error);
    }
  },

  initNotificationListeners: () => {
    notificationListenerRefCount += 1;

    if (!notificationListenersInitPromise && !notificationListenersCleanup) {
      notificationListenersInitPromise = Promise.all([
        onNotificationCenterChanged(() => {
          void get().fetchNotifications();
        }),
        onTransientNotification((notification) => {
          set((state) => {
            const transientNotifications = [
              ...state.transientNotifications.filter((item) => item.id !== notification.id),
              notification,
            ];
            return buildDerivedState(state.persistedNotifications, transientNotifications);
          });
        }),
      ])
        .then((unlisteners) => {
          notificationListenersCleanup = () => {
            unlisteners.forEach((unlisten) => unlisten());
          };
          notificationListenersInitPromise = null;

          if (notificationListenerRefCount === 0) {
            releaseNotificationListeners();
          }
        })
        .catch((error) => {
          console.error("Failed to initialize notification listeners:", error);
          notificationListenersInitPromise = null;
          notificationListenersCleanup = null;
        });
    }

    let released = false;

    return () => {
      if (released) {
        return;
      }

      released = true;
      notificationListenerRefCount = Math.max(0, notificationListenerRefCount - 1);

      if (notificationListenerRefCount === 0 && notificationListenersCleanup) {
        releaseNotificationListeners();
      }
    };
  },
}));
