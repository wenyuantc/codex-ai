import { useEffect, useState } from "react";
import { Download, Loader2 } from "lucide-react";

import { useDashboardStore } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { DashboardStats } from "@/components/dashboard/DashboardStats";
import { ActivityFeed } from "@/components/dashboard/ActivityFeed";
import { EmployeePerformanceChart } from "@/components/dashboard/EmployeePerformanceChart";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  exportTasksCsv,
  getDashboardReportSummary,
  type DashboardReportSummary,
} from "@/lib/backend";
import { TASK_STATUSES } from "@/lib/types";
import { getStatusLabel, getStatusColor } from "@/lib/utils";

export function DashboardPage() {
  const { stats, fetchStats } = useDashboardStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const [report, setReport] = useState<DashboardReportSummary | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const taskDistribution = stats?.tasksByStatus
    ? TASK_STATUSES.map((status) => ({
        status,
        count: stats.tasksByStatus[status.value] ?? 0,
      }))
    : [];
  const maxTaskCount = Math.max(...taskDistribution.map((item) => item.count), 0);
  const maxWeekly = Math.max(...(report?.weekly_completed.map((p) => p.count) ?? [0]), 0);

  useEffect(() => {
    void fetchStats(environmentMode, selectedSshConfigId, currentProjectId);
  }, [currentProjectId, environmentMode, fetchStats, selectedSshConfigId]);

  useEffect(() => {
    void getDashboardReportSummary({
      projectId: currentProjectId,
      environmentMode,
    })
      .then(setReport)
      .catch(() => setReport(null));
  }, [currentProjectId, environmentMode]);

  const handleExportCsv = async () => {
    setExporting(true);
    setExportMessage(null);
    try {
      const result = await exportTasksCsv({
        projectId: currentProjectId,
        environmentMode,
      });
      const blob = new Blob([result.csv], { type: "text/csv;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `tasks-export-${new Date().toISOString().slice(0, 10)}.csv`;
      anchor.click();
      URL.revokeObjectURL(url);
      setExportMessage(`已导出 ${result.row_count} 条任务。`);
    } catch (error) {
      setExportMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-end gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleExportCsv()}
          disabled={exporting}
        >
          {exporting ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          导出任务 CSV
        </Button>
        {exportMessage && <p className="text-[11px] text-muted-foreground">{exportMessage}</p>}
      </div>
      <DashboardStats />

      {report && (
        <Card className="p-4 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-semibold">增强报表</h3>
            <p className="text-xs text-muted-foreground">
              完成率 {report.completion_rate.toFixed(1)}% · 逾期 {report.overdue_tasks} · 阻塞{" "}
              {report.blocked_tasks}
            </p>
          </div>
          <div className="grid gap-4 lg:grid-cols-2">
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">近 7 日完成趋势</p>
              <div className="flex h-28 items-end gap-2">
                {report.weekly_completed.map((point) => {
                  const height = maxWeekly > 0 ? (point.count / maxWeekly) * 100 : 0;
                  return (
                    <div key={point.label} className="flex flex-1 flex-col items-center gap-1">
                      <span className="text-[10px] font-medium">{point.count}</span>
                      <div className="flex h-20 w-full items-end rounded-sm bg-muted/40 px-0.5">
                        <div
                          className="w-full rounded-t bg-primary/80 transition-all"
                          style={{ height: point.count > 0 ? `${Math.max(height, 8)}%` : "0%" }}
                        />
                      </div>
                      <span className="text-[10px] text-muted-foreground">{point.label}</span>
                    </div>
                  );
                })}
              </div>
            </div>
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                成员负载（进行中 vs 已完成）
              </p>
              <div className="max-h-28 space-y-1.5 overflow-auto">
                {report.employee_workload.length === 0 && (
                  <p className="text-xs text-muted-foreground">暂无员工数据</p>
                )}
                {report.employee_workload.map((item) => (
                  <div
                    key={item.employee_id}
                    className="flex items-center justify-between gap-2 text-xs"
                  >
                    <span className="truncate font-medium">{item.employee_name}</span>
                    <span className="shrink-0 text-muted-foreground">
                      活跃 {item.active_tasks} · 完成 {item.completed_tasks}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </Card>
      )}

      {/* Task Distribution */}
      {stats?.tasksByStatus && (
        <Card className="p-4">
          <h3 className="text-sm font-semibold mb-3">任务分布</h3>
          <div className="grid grid-cols-[repeat(auto-fit,minmax(4.75rem,1fr))] items-end gap-3">
            {taskDistribution.map(({ status, count }) => {
              const height = maxTaskCount > 0 ? (count / maxTaskCount) * 100 : 0;
              const label = getStatusLabel(status.value);

              return (
                <div
                  key={status.value}
                  className="flex min-w-0 flex-col items-center gap-1"
                  title={`${label}: ${count} 个任务`}
                >
                  <span className="text-sm font-bold">{count}</span>
                  <div className="flex h-[clamp(5rem,16vw,7rem)] w-full items-end justify-center rounded-sm bg-muted/30 px-1">
                    <div
                      className={`w-full max-w-9 rounded-t sm:max-w-10 ${getStatusColor(status.value)} transition-all`}
                      style={{ height: count > 0 ? `${Math.max(height, 6)}%` : "0%" }}
                    />
                  </div>
                  <span className="w-full truncate text-center text-[10px] text-muted-foreground">
                    {label}
                  </span>
                </div>
              );
            })}
          </div>
        </Card>
      )}

      {/* Two-column: Activity Feed + Performance */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <ActivityFeed />
        <EmployeePerformanceChart />
      </div>
    </div>
  );
}
