import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useNotificationStore } from "@/stores/notificationStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEffect } from "react";

export function MainLayout() {
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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      switch (e.key.toLowerCase()) {
        case "n":
          e.preventDefault();
          window.dispatchEvent(new CustomEvent("shortcut:new-task"));
          break;
        case "e":
          e.preventDefault();
          window.dispatchEvent(new CustomEvent("shortcut:toggle-employees"));
          break;
        case "k":
          e.preventDefault();
          window.dispatchEvent(new CustomEvent("shortcut:command-palette"));
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar />
      <div className="flex flex-col flex-1 overflow-hidden">
        <Header />
        <main className="flex-1 overflow-auto p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
