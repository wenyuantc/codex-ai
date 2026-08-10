import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CheckCircle2, Circle, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { healthCheck } from "@/lib/backend";
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
  const navigate = useNavigate();
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const projects = useProjectStore((state) => state.projects);
  const sshConfigs = useProjectStore((state) => state.sshConfigs);
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const employees = useEmployeeStore((state) => state.employees);
  const tasks = useTaskStore((state) => state.tasks);
  const fetchTasks = useTaskStore((state) => state.fetchTasks);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);

  const [dismissed, setDismissed] = useState(readDismissed);
  const [sdkReady, setSdkReady] = useState<boolean | null>(null);

  const refreshHealth = useCallback(() => {
    void healthCheck()
      .then((health) => {
        setSdkReady(Boolean(health.sdk_installed || health.codex_available));
      })
      .catch(() => setSdkReady(false));
  }, []);

  useEffect(() => {
    refreshHealth();
  }, [refreshHealth, environmentMode]);

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
        title: "检查 AI SDK / CLI",
        description: "确认 Codex（或其他引擎）可用，才能运行任务。",
        done: sdkReady === true,
        actionLabel: "打开运行时设置",
        onAction: () => navigate("/settings?section=sdk"),
      },
    ];

    if (environmentMode === "ssh") {
      list.push({
        id: "ssh",
        title: "配置 SSH 主机",
        description: "SSH 模式下需要至少一台可用远程主机。",
        done: hasSshConfig,
        actionLabel: "配置 SSH",
        onAction: () => navigate("/settings?section=ssh"),
      });
    }

    list.push(
      {
        id: "project",
        title: "创建项目",
        description: environmentMode === "ssh" ? "添加一个 SSH 类型项目。" : "添加一个本地项目。",
        done: hasProject,
        actionLabel: "去项目管理",
        onAction: () => navigate("/projects"),
      },
      {
        id: "employee",
        title: "创建 AI 员工",
        description: "为项目绑定开发/审核/测试/协调员角色。",
        done: hasEmployee,
        actionLabel: "去员工管理",
        onAction: () => navigate("/employees"),
      },
      {
        id: "task",
        title: "创建并运行首个任务",
        description: "在看板新建任务，指派员工后即可运行。",
        done: hasTask,
        actionLabel: "打开看板",
        onAction: () => navigate("/kanban"),
      },
    );

    return list;
  }, [environmentMode, hasEmployee, hasProject, hasSshConfig, hasTask, navigate, sdkReady]);

  const allDone = steps.every((step) => step.done);
  const completedCount = steps.filter((step) => step.done).length;

  if (dismissed) {
    return null;
  }

  if (allDone && sdkReady !== null) {
    return (
      <Card className="flex items-start justify-between gap-3 border-emerald-500/30 bg-emerald-500/5 p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold">首次设置已完成</h3>
          <p className="text-xs text-muted-foreground">
            可以在看板继续创建任务，或在设置中开启测试员自动化与自动质控。
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label="关闭首次引导"
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
          <h3 className="text-sm font-semibold">开始使用 Codex AI</h3>
          <p className="text-xs text-muted-foreground">
            完成以下步骤即可跑通「项目 → 员工 → 任务 → AI 执行」主路径（{completedCount}/
            {steps.length}）
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          aria-label="关闭首次引导"
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
