import type { NavigateFunction } from "react-router-dom";

import { useProjectStore } from "@/stores/projectStore";

export interface NotificationOpenTarget {
  action_route: string | null;
  project_id: string | null;
  ssh_config_id: string | null;
}

export async function openNotificationTarget(
  navigate: NavigateFunction,
  notification: NotificationOpenTarget,
) {
  if (notification.ssh_config_id) {
    const {
      environmentMode,
      setEnvironmentMode,
      setSelectedSshConfigId,
    } = useProjectStore.getState();

    if (environmentMode !== "ssh") {
      await setEnvironmentMode("ssh");
    }

    setSelectedSshConfigId(notification.ssh_config_id);
  }

  if (notification.project_id) {
    const {
      allProjects,
      environmentMode,
      setCurrentProject,
      setEnvironmentMode,
      setSelectedSshConfigId,
    } = useProjectStore.getState();
    const targetProject = allProjects.find((project) => project.id === notification.project_id) ?? null;

    if (targetProject?.project_type === "ssh") {
      if (environmentMode !== "ssh") {
        await setEnvironmentMode("ssh");
      }

      if (targetProject.ssh_config_id) {
        setSelectedSshConfigId(targetProject.ssh_config_id);
      }
    } else if (targetProject?.project_type === "local" && environmentMode !== "local") {
      await setEnvironmentMode("local");
    }

    setCurrentProject(targetProject);
  }

  if (!notification.action_route) {
    return;
  }

  navigate(notification.action_route, {
    state: { notificationNonce: Date.now() },
  });
}
