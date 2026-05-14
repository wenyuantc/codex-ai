import { useEffect, useMemo, useState } from "react";
import { select } from "@/lib/database";
import { filterProjectsByScope, normalizeProject } from "@/lib/projects";
import type { EmployeeMetric, EnvironmentMode, Project } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/projectStore";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ArrowDown, ArrowUp, ArrowUpDown, BarChart3 } from "lucide-react";

type PerformancePeriod = "7" | "30" | "90";
type SortDirection = "asc" | "desc";
type SortKey = "employee_name" | "tasks_completed" | "average_completion_time" | "success_rate";

interface PerformanceRow {
  employee_id: string;
  employee_name: string;
  tasks_completed: number;
  average_completion_time: number | null;
  success_rate: number | null;
}

interface EmployeeLookup {
  id: string;
  name: string;
  project_id: string | null;
}

interface SortState {
  key: SortKey;
  direction: SortDirection;
}

interface SortHeaderProps {
  label: string;
  sortKey: SortKey;
  sort: SortState;
  align?: "left" | "right";
  onSort: (key: SortKey) => void;
}

const PERIOD_OPTIONS: { value: PerformancePeriod; label: string }[] = [
  { value: "7", label: "近7天" },
  { value: "30", label: "近30天" },
  { value: "90", label: "近90天" },
];

const DEFAULT_SORT_DIRECTIONS: Record<SortKey, SortDirection> = {
  employee_name: "asc",
  tasks_completed: "desc",
  average_completion_time: "asc",
  success_rate: "desc",
};

function getPeriodLabel(value: PerformancePeriod) {
  return PERIOD_OPTIONS.find((option) => option.value === value)?.label ?? value;
}

function compareNullableNumbers(left: number | null, right: number | null, direction: SortDirection) {
  if (left === null && right === null) {
    return 0;
  }
  if (left === null) {
    return 1;
  }
  if (right === null) {
    return -1;
  }
  const result = left - right;
  return direction === "asc" ? result : -result;
}

function compareRows(left: PerformanceRow, right: PerformanceRow, sort: SortState) {
  let result = 0;

  if (sort.key === "employee_name") {
    result = left.employee_name.localeCompare(right.employee_name, "zh-CN");
  } else if (sort.key === "tasks_completed") {
    result = left.tasks_completed - right.tasks_completed;
  } else if (sort.key === "average_completion_time") {
    result = compareNullableNumbers(left.average_completion_time, right.average_completion_time, sort.direction);
  } else {
    result = compareNullableNumbers(left.success_rate, right.success_rate, sort.direction);
  }

  if (result === 0) {
    return left.employee_name.localeCompare(right.employee_name, "zh-CN");
  }

  if (sort.key === "average_completion_time" || sort.key === "success_rate") {
    return result;
  }

  return sort.direction === "asc" ? result : -result;
}

function SortHeader({ label, sortKey, sort, align = "left", onSort }: SortHeaderProps) {
  const active = sort.key === sortKey;
  const Icon = active
    ? sort.direction === "asc"
      ? ArrowUp
      : ArrowDown
    : ArrowUpDown;

  return (
    <Button
      type="button"
      variant="ghost"
      size="xs"
      className={cn(
        "h-auto px-1 py-0.5 text-xs font-medium text-muted-foreground hover:text-foreground",
        align === "right" && "ml-auto",
      )}
      onClick={() => onSort(sortKey)}
      aria-label={`按${label}排序`}
    >
      {label}
      <Icon className="h-3 w-3" />
    </Button>
  );
}

async function loadPerformanceRows(
  days: PerformancePeriod,
  projectId: string | undefined,
  environmentMode: EnvironmentMode,
  selectedSshConfigId?: string | null,
) {
  const [metrics, employees, projects] = await Promise.all([
    select<EmployeeMetric>(
      `SELECT * FROM employee_metrics
       WHERE period_start >= datetime('now', ?)
       ORDER BY tasks_completed DESC`,
      [`-${days} days`],
    ),
    select<EmployeeLookup>("SELECT id, name, project_id FROM employees"),
    select<Project>("SELECT * FROM projects WHERE deleted_at IS NULL"),
  ]);
  const visibleProjectIds = new Set(
    filterProjectsByScope(
      projects.map((project) => normalizeProject(project)),
      environmentMode,
      selectedSshConfigId,
    ).map((project) => project.id),
  );
  const filteredEmployees = employees.filter((employee) => {
    if (!projectId) {
      return employee.project_id ? visibleProjectIds.has(employee.project_id) : false;
    }
    return employee.project_id === projectId;
  });

  const empMap = new Map(filteredEmployees.map((employee) => [employee.id, employee.name]));

  const aggregated = new Map<string, PerformanceRow>();
  for (const metric of metrics) {
    const employeeName = empMap.get(metric.employee_id);
    if (!employeeName) {
      continue;
    }

    const existing = aggregated.get(metric.employee_id);
    if (existing) {
      existing.tasks_completed += metric.tasks_completed;
      if (metric.success_rate !== null) {
        existing.success_rate = existing.success_rate === null
          ? metric.success_rate
          : (existing.success_rate + metric.success_rate) / 2;
      }
      if (metric.average_completion_time !== null) {
        existing.average_completion_time = existing.average_completion_time === null
          ? metric.average_completion_time
          : (existing.average_completion_time + metric.average_completion_time) / 2;
      }
    } else {
      aggregated.set(metric.employee_id, {
        employee_id: metric.employee_id,
        employee_name: employeeName,
        tasks_completed: metric.tasks_completed,
        average_completion_time: metric.average_completion_time,
        success_rate: metric.success_rate,
      });
    }
  }

  return Array.from(aggregated.values());
}

export function EmployeePerformanceChart() {
  const [data, setData] = useState<PerformanceRow[]>([]);
  const [period, setPeriod] = useState<PerformancePeriod>("30");
  const [sort, setSort] = useState<SortState>({ key: "tasks_completed", direction: "desc" });
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);

  useEffect(() => {
    let active = true;

    void loadPerformanceRows(period, currentProjectId, environmentMode, selectedSshConfigId)
      .then((rows) => {
        if (active) {
          setData(rows);
        }
      })
      .catch((error) => {
        console.error("Failed to fetch performance data:", error);
        if (active) {
          setData([]);
        }
      });

    return () => {
      active = false;
    };
  }, [currentProjectId, environmentMode, period, selectedSshConfigId]);

  const sortedData = useMemo(() => (
    [...data].sort((left, right) => compareRows(left, right, sort))
  ), [data, sort]);

  const updateSort = (key: SortKey) => {
    setSort((current) => {
      if (current.key === key) {
        return {
          key,
          direction: current.direction === "asc" ? "desc" : "asc",
        };
      }

      return {
        key,
        direction: DEFAULT_SORT_DIRECTIONS[key],
      };
    });
  };

  return (
    <Card className="p-4 flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <BarChart3 className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-sm font-semibold">员工绩效</h3>
        </div>
        <Select<PerformancePeriod>
          value={period}
          onValueChange={(value) => {
            if (value) {
              setPeriod(value);
            }
          }}
        >
          <SelectTrigger className="h-7 w-24 bg-background text-xs">
            <SelectValue>
              {(value) => (typeof value === "string" ? getPeriodLabel(value as PerformancePeriod) : getPeriodLabel(period))}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {PERIOD_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {sortedData.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-8">
          暂无绩效数据
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border">
                <th className="py-2 px-2 text-left">
                  <SortHeader label="员工" sortKey="employee_name" sort={sort} onSort={updateSort} />
                </th>
                <th className="py-2 px-2 text-right">
                  <SortHeader label="完成任务" sortKey="tasks_completed" sort={sort} align="right" onSort={updateSort} />
                </th>
                <th className="py-2 px-2 text-right">
                  <SortHeader label="平均耗时" sortKey="average_completion_time" sort={sort} align="right" onSort={updateSort} />
                </th>
                <th className="py-2 px-2 text-right">
                  <SortHeader label="成功率" sortKey="success_rate" sort={sort} align="right" onSort={updateSort} />
                </th>
              </tr>
            </thead>
            <tbody>
              {sortedData.map((row) => (
                <tr key={row.employee_id} className="border-b border-border/50 last:border-0">
                  <td className="py-2 px-2 font-medium">{row.employee_name}</td>
                  <td className="py-2 px-2 text-right">{row.tasks_completed}</td>
                  <td className="py-2 px-2 text-right text-muted-foreground">
                    {row.average_completion_time !== null
                      ? `${Math.round(row.average_completion_time)}s`
                      : "-"}
                  </td>
                  <td className="py-2 px-2 text-right">
                    {row.success_rate !== null ? (
                      <span
                        className={
                          row.success_rate >= 80
                            ? "text-green-500"
                            : row.success_rate >= 50
                              ? "text-yellow-500"
                              : "text-red-500"
                        }
                      >
                        {Math.round(row.success_rate)}%
                      </span>
                    ) : (
                      <span className="text-muted-foreground">-</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  );
}
