import type { ReactNode } from "react";
import { Link } from "react-router-dom";

export const EMPLOYEE_ROLE_OPTIONS = [
  {
    value: "developer",
    label: "开发者",
    hint: "负责实现任务、编写代码并推进执行。",
  },
  {
    value: "reviewer",
    label: "审查员",
    hint: "负责代码审核，输出问题与修复建议。",
  },
  {
    value: "tester",
    label: "测试员",
    hint: "负责验收测试；可为任务生成验收清单。开启「测试员自动化」后，任务执行完成会自动验收再进入审核。",
    settingsHint: (
      <>
        开关位置：
        <Link className="mx-1 underline" to="/settings?section=git">
          设置 → Git 与自动质控
        </Link>
      </>
    ),
  },
  {
    value: "coordinator",
    label: "协调员",
    hint: "负责任务拆解与执行计划；可在看板生成协调员计划并按计划编排执行。",
    settingsHint: <>入口：看板任务详情 / 右键「协调员计划」与「按计划编排」。</>,
  },
] as const;

export type EmployeeRoleValue = (typeof EMPLOYEE_ROLE_OPTIONS)[number]["value"];

export function getEmployeeRoleOption(role: string) {
  return EMPLOYEE_ROLE_OPTIONS.find((option) => option.value === role);
}

export function EmployeeRoleHint({ role }: { role: string }): ReactNode {
  const option = getEmployeeRoleOption(role);
  if (!option) {
    return null;
  }
  return (
    <div className="space-y-1 text-xs text-muted-foreground">
      <p>{option.hint}</p>
      {"settingsHint" in option && option.settingsHint ? <p>{option.settingsHint}</p> : null}
    </div>
  );
}
