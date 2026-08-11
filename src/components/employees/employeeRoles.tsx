import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";

export const EMPLOYEE_ROLE_OPTIONS = [
  { value: "developer", labelKey: "employees:roleHint.developer" },
  { value: "reviewer", labelKey: "employees:roleHint.reviewer" },
  { value: "tester", labelKey: "employees:roleHint.tester" },
  { value: "coordinator", labelKey: "employees:roleHint.coordinator" },
] as const;

export type EmployeeRoleValue = (typeof EMPLOYEE_ROLE_OPTIONS)[number]["value"];

export function getEmployeeRoleOption(role: string) {
  return EMPLOYEE_ROLE_OPTIONS.find((option) => option.value === role);
}

export function EmployeeRoleHint({ role }: { role: string }): ReactNode {
  const { t } = useTranslation(["employees", "common"]);
  const option = getEmployeeRoleOption(role);
  if (!option) {
    return null;
  }
  return (
    <div className="space-y-1 text-xs text-muted-foreground">
      <p>{t(option.labelKey)}</p>
      {role === "tester" ? (
        <p>
          {t("roleSettingsHint.prefix")}
          <Link className="mx-1 underline" to="/settings?section=git">
            {t("roleSettingsHint.link")}
          </Link>
        </p>
      ) : null}
      {role === "coordinator" ? <p>{t("roleSettingsHint.coordinator")}</p> : null}
    </div>
  );
}
