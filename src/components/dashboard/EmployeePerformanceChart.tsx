import { useEffect, useState } from "react";
import { select } from "@/lib/database";
import type { EmployeeMetric } from "@/lib/types";
import { useProjectStore } from "@/stores/projectStore";
import { Card } from "@/components/ui/card";
import { BarChart3 } from "lucide-react";
import { filterProjectsByScope, normalizeProject } from "@/lib/projects";
import type { Project } from "@/lib/types";

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

export function EmployeePerformanceChart() {
  const [data, setData] = useState<PerformanceRow[]>([]);
  const [period, setPeriod] = useState<"7" | "30" | "90">("30");
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);

  useEffect(() => {
    void fetchData(period, currentProjectId, environmentMode, selectedSshConfigId);
  }, [currentProjectId, environmentMode, period, selectedSshConfigId]);

  const fetchData = async (
    days: string,
    projectId?: string,
    nextEnvironmentMode?: "local" | "ssh",
    nextSelectedSshConfigId?: string | null,
  ) => {
    try {
      const [metrics, employees, projects] = await Promise.all([
        select<EmployeeMetric>(
          `SELECT * FROM employee_metrics
           WHERE period_start >= datetime('now', '-${days} days')
           ORDER BY tasks_completed DESC`
        ),
        select<EmployeeLookup>("SELECT id, name, project_id FROM employees"),
        select<Project>("SELECT * FROM projects"),
      ]);
      const visibleProjectIds = new Set(
        filterProjectsByScope(
          projects.map((project) => normalizeProject(project)),
          nextEnvironmentMode ?? "local",
          nextSelectedSshConfigId,
        ).map((project) => project.id),
      );
      const filteredEmployees = employees.filter((employee) => {
        if (!projectId) {
          return employee.project_id ? visibleProjectIds.has(employee.project_id) : false;
        }
        return employee.project_id === projectId;
      });

      const empMap = new Map(filteredEmployees.map((e) => [e.id, e.name]));

      // Aggregate by employee
      const aggregated = new Map<string, PerformanceRow>();
      for (const m of metrics) {
        const employeeName = empMap.get(m.employee_id);
        if (!employeeName) {
          continue;
        }

        const existing = aggregated.get(m.employee_id);
        if (existing) {
          existing.tasks_completed += m.tasks_completed;
          if (m.success_rate !== null) {
            existing.success_rate = existing.success_rate === null
              ? m.success_rate
              : (existing.success_rate + m.success_rate) / 2;
          }
          if (m.average_completion_time !== null) {
            existing.average_completion_time = existing.average_completion_time === null
              ? m.average_completion_time
              : (existing.average_completion_time + m.average_completion_time) / 2;
          }
        } else {
          aggregated.set(m.employee_id, {
            employee_id: m.employee_id,
            employee_name: employeeName,
            tasks_completed: m.tasks_completed,
            average_completion_time: m.average_completion_time,
            success_rate: m.success_rate,
          });
        }
      }

      setData(Array.from(aggregated.values()));
    } catch (e) {
      console.error("Failed to fetch performance data:", e);
      setData([]);
    }
  };

  return (
    <Card className="p-4 flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <BarChart3 className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-sm font-semibold">员工绩效</h3>
        </div>
        <select
          value={period}
          onChange={(e) => setPeriod(e.target.value as "7" | "30" | "90")}
          className="text-xs border border-input rounded px-1.5 py-0.5 bg-background"
        >
          <option value="7">近7天</option>
          <option value="30">近30天</option>
          <option value="90">近90天</option>
        </select>
      </div>

      {data.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-8">
          暂无绩效数据
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 px-2 text-xs font-medium text-muted-foreground">员工</th>
                <th className="text-right py-2 px-2 text-xs font-medium text-muted-foreground">完成任务</th>
                <th className="text-right py-2 px-2 text-xs font-medium text-muted-foreground">平均耗时</th>
                <th className="text-right py-2 px-2 text-xs font-medium text-muted-foreground">成功率</th>
              </tr>
            </thead>
            <tbody>
              {data.map((row) => (
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
