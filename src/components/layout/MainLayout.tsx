import { Outlet, useNavigate } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useNotificationStore } from "@/stores/notificationStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEffect, useEffectEvent } from "react";
import { showMainWindow } from "@/lib/backend";
import { initDesktopNotificationBridge } from "@/lib/desktopNotifications";
import { openNotificationTarget } from "@/lib/notificationNavigation";
import type { DesktopNotificationExtra } from "@/lib/types";

export function MainLayout() {
  const navigate = useNavigate();
  const initCodexListeners = useEmployeeStore((s) => s.initCodexListeners);
  const initCodexSessionListeners = useTaskStore((s) => s.initCodexSessionListeners);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const initNotificationListeners = useNotificationStore((state) => state.initNotificationListeners);
  const fetchNotifications = useNotificationStore((state) => state.fetchNotifications);
  const syncSystemNotifications = useNotificationStore((state) => state.syncSystemNotifications);

  useEffect(() => {
    const cleanup = initCodexListeners();
    return cleanup;
  }, [initCodexListeners]);

  useEffect(() => {
    const cleanup = initCodexSessionListeners();
    return cleanup;
  }, [initCodexSessionListeners]);

  useEffect(() => {
    const cleanup = initNotificationListeners();
    return cleanup;
  }, [initNotificationListeners]);

  const handleDesktopNotificationOpen = useEffectEvent(async (payload: DesktopNotificationExtra) => {
    await useNotificationStore.getState().markRead(payload.notification_id);
    await showMainWindow();
    await openNotificationTarget(navigate, payload);
  });

  useEffect(() => {
    const cleanup = initDesktopNotificationBridge(handleDesktopNotificationOpen);
    return cleanup;
  }, [handleDesktopNotificationOpen]);

  useEffect(() => {
    void syncSystemNotifications(environmentMode, selectedSshConfigId);
    void fetchNotifications();

    const sync = () => {
      void syncSystemNotifications(environmentMode, selectedSshConfigId);
      void fetchNotifications();
    };

    const interval = window.setInterval(sync, 60000);
    const handleFocus = () => sync();

    window.addEventListener("focus", handleFocus);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", handleFocus);
    };
  }, [environmentMode, fetchNotifications, selectedSshConfigId, syncSystemNotifications]);

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <Sidebar />
      <div className="flex flex-col flex-1 overflow-hidden bg-background">
        <Header />
        <main className="flex-1 overflow-auto bg-background p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
