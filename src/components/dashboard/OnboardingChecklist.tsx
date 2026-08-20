import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { CheckCircle2, Circle, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  isAnyEngineReady,
  probeEngineReadiness,
  readyEngineIds,
  type EngineReadyFlags,
} from "@/lib/engineHealth";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";

const DISMISS_STORAGE_KEY = "codex-ai:onboarding-checklist-dismissed";

type StepId = "sdk" | "ssh" | "project" | "employee" | "task";

interface ChecklistStep {
  id: StepId;
  title: string;
  description: string;
  done: boolean;
  actionLabel: string;
  onAction: () => void;
}

function readDismissed(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(DISMISS_STORAGE_KEY) === "1";
}

function writeDismissed() {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(DISMISS_STORAGE_KEY, "1");
}

export function OnboardingChecklist() {
  const { t } = useTranslation("dashboard");
  const navigate = useNavigate();
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const projects = useProjectStore((state) => state.projects);
  const sshConfigs = useProjectStore((state) => state.sshConfigs);
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const employees = useEmployeeStore((state) => state.employees);
  const tasks = useTaskStore((state) => state.tasks);
  const fetchTasks = useTaskStore((state) => state.fetchTasks);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);

  const [dismissed, setDismissed] = useState(readDismissed);
  const [engineFlags, setEngineFlags] = useState<EngineReadyFlags | null>(null);
  const sdkReady = engineFlags ? isAnyEngineReady(engineFlags) : null;
  const sshConfigId = selectedSshConfigId ?? sshConfigs[0]?.id ?? null;

  const refreshHealth = useCallback(() => {
    void probeEngineReadiness({ environmentMode, sshConfigId })
      .then(setEngineFlags)
      .catch(() => setEngineFlags({ codex: false, claude: false, grok: false, opencode: false }));
  }, [environmentMode, sshConfigId]);

  useEffect(() => {
    refreshHealth();
  }, [refreshHealth]);

  useEffect(() => {
    void fetchEmployees();
    void fetchTasks(currentProjectId);
  }, [currentProjectId, fetchEmployees, fetchTasks]);

  const hasProject = projects.length > 0;
  const hasSshConfig = sshConfigs.length > 0;
  const scopedEmployees = currentProjectId
    ? employees.filter((employee) => employee.project_id === currentProjectId)
    : employees;
  const hasEmployee = scopedEmployees.length > 0 || (!currentProjectId && employees.length > 0);
  const hasTask = tasks.some((task) => task.status !== "archived");

  const steps = useMemo<ChecklistStep[]>(() => {
    const list: ChecklistStep[] = [
      {
        id: "sdk",
        title: t("onboarding.checkSdkTitle"),
        description:
          sdkReady && engineFlags
            ? t("onboarding.checkSdkDescriptionReady", {
                engines: readyEngineIds(engineFlags)
                  .map((id) => t(`onboarding.engine.${id}`))
                  .join(t("onboarding.engineSeparator")),
              })
            : environmentMode === "ssh"
              ? t("onboarding.checkSdkDescriptionSsh")
              : t("onboarding.checkSdkDescription"),
        done: sdkReady === true,
        actionLabel: t("onboarding.checkSdkAction"),
        onAction: () => navigate("/settings?section=sdk"),
      },
    ];

    if (environmentMode === "ssh") {
      list.push({
        id: "ssh",
        title: t("onboarding.sshTitle"),
        description: t("onboarding.sshDescription"),
        done: hasSshConfig,
        actionLabel: t("onboarding.sshAction"),
        onAction: () => navigate("/settings?section=ssh"),
      });
    }

    list.push(
      {
        id: "project",
        title: t("onboarding.projectTitle"),
        description:
          environmentMode === "ssh"
            ? t("onboarding.projectDescriptionSsh")
            : t("onboarding.projectDescriptionLocal"),
        done: hasProject,
        actionLabel: t("onboarding.projectAction"),
        onAction: () => navigate("/projects"),
      },
      {
        id: "employee",
        title: t("onboarding.employeeTitle"),
        description: t("onboarding.employeeDescription"),
        done: hasEmployee,
        actionLabel: t("onboarding.employeeAction"),
        onAction: () => navigate("/employees"),
      },
      {
        id: "task",
        title: t("onboarding.taskTitle"),
        description: t("onboarding.taskDescription"),
        done: hasTask,
        actionLabel: t("onboarding.taskAction"),
        onAction: () => navigate("/kanban"),
      },
    );

    return list;
  }, [
    engineFlags,
    environmentMode,
    hasEmployee,
    hasProject,
    hasSshConfig,
    hasTask,
    navigate,
    sdkReady,
    t,
  ]);

  const allDone = steps.every((step) => step.done);
  const completedCount = steps.filter((step) => step.done).length;

  if (dismissed) {
    return null;
  }

  if (allDone && sdkReady !== null) {
    return (
      <Card className="flex items-start justify-between gap-3 border-emerald-500/30 bg-emerald-500/5 p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold">{t("onboarding.doneTitle")}</h3>
          <p className="text-xs text-muted-foreground">{t("onboarding.doneDescription")}</p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label={t("onboarding.closeAria")}
          onClick={() => {
            writeDismissed();
            setDismissed(true);
          }}
        >
          <X className="h-4 w-4" />
        </Button>
      </Card>
    );
  }

  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold">{t("onboarding.startTitle")}</h3>
          <p className="text-xs text-muted-foreground">
            {t("onboarding.startDescription", {
              completed: completedCount,
              total: steps.length,
            })}
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label={t("onboarding.closeAria")}
          onClick={() => {
            writeDismissed();
            setDismissed(true);
          }}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>

      <ul className="space-y-2">
        {steps.map((step) => (
          <li
            key={step.id}
            className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border px-3 py-2"
          >
            <div className="flex min-w-0 items-start gap-2">
              {step.done ? (
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-500" />
              ) : (
                <Circle className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
              )}
              <div className="min-w-0 space-y-0.5">
                <p className="text-sm font-medium">{step.title}</p>
                <p className="text-xs text-muted-foreground">{step.description}</p>
              </div>
            </div>
            {!step.done && (
              <Button variant="outline" size="sm" onClick={step.onAction}>
                {step.actionLabel}
              </Button>
            )}
          </li>
        ))}
      </ul>
    </Card>
  );
}
