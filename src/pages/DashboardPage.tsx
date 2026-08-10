import { useCallback, useEffect, useRef, useState, type ChangeEvent } from "react";
import { Download, Loader2, Upload } from "lucide-react";

import { useDashboardStore } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { DashboardStats } from "@/components/dashboard/DashboardStats";
import { ActivityFeed } from "@/components/dashboard/ActivityFeed";
import { EmployeePerformanceChart } from "@/components/dashboard/EmployeePerformanceChart";
import { OnboardingChecklist } from "@/components/dashboard/OnboardingChecklist";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  exportTasksCsv,
  exportTasksJson,
  getDashboardReportSummary,
  importTasksJson,
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
  const [reportLoading, setReportLoading] = useState(false);
  const [reportError, setReportError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportingCsv, setExportingCsv] = useState(false);
  const [importing, setImporting] = useState(false);
  const [ioMessage, setIoMessage] = useState<string | null>(null);
  const importInputRef = useRef<HTMLInputElement>(null);
  const taskDistribution = stats?.tasksByStatus
    ? TASK_STATUSES.map((status) => ({
        status,
        count: stats.tasksByStatus[status.value] ?? 0,
      }))
    : [];
  const maxTaskCount = Math.max(...taskDistribution.map((item) => item.count), 0);
  const maxDaily = Math.max(...(report?.weekly_completed.map((p) => p.count) ?? [0]), 0);
  const maxWeeklySeries = Math.max(
    ...(report?.weekly_completed_series?.map((p) => p.count) ?? [0]),
    0,
  );

  const loadReport = useCallback(async () => {
    setReportLoading(true);
    setReportError(null);
    try {
      const summary = await getDashboardReportSummary({
        projectId: currentProjectId,
        environmentMode,
        selectedSshConfigId,
      });
      setReport(summary);
    } catch (error) {
      setReport(null);
      setReportError(error instanceof Error ? error.message : String(error));
    } finally {
      setReportLoading(false);
    }
  }, [currentProjectId, environmentMode, selectedSshConfigId]);

  useEffect(() => {
    void fetchStats(environmentMode, selectedSshConfigId, currentProjectId);
  }, [currentProjectId, environmentMode, fetchStats, selectedSshConfigId]);

  useEffect(() => {
    void loadReport();
  }, [loadReport]);

  const handleExportJson = async () => {
    setExporting(true);
    setIoMessage(null);
    try {
      const result = await exportTasksJson({
        projectId: currentProjectId,
        environmentMode,
        selectedSshConfigId,
      });
      const blob = new Blob([result.json], { type: "application/json;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `tasks-export-${new Date().toISOString().slice(0, 10)}.json`;
      anchor.click();
      URL.revokeObjectURL(url);
      setIoMessage(
        `已导出 ${result.task_count} 条任务 JSON${result.truncated ? "（已截断至上限）" : ""}。`,
      );
    } catch (error) {
      setIoMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setExporting(false);
    }
  };

  const handleExportCsv = async () => {
    setExportingCsv(true);
    setIoMessage(null);
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
      setIoMessage(`已导出 ${result.row_count} 行任务 CSV。`);
    } catch (error) {
      setIoMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingCsv(false);
    }
  };

  const handleImportClick = () => {
    if (!currentProjectId) {
      setIoMessage("请先选择目标项目，再导入任务 JSON。");
      return;
    }
    importInputRef.current?.click();
  };

  const handleImportFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    if (!currentProjectId) {
      setIoMessage("请先选择目标项目，再导入任务 JSON。");
      return;
    }

    setImporting(true);
    setIoMessage(null);
    try {
      const text = await file.text();
      const result = await importTasksJson({
        projectId: currentProjectId,
        json: text,
        conflictStrategy: "create_new",
      });
      if (result.failed > 0) {
        const firstError = result.errors[0]?.message ?? "校验失败";
        setIoMessage(
          `导入失败：${firstError}${result.failed > 1 ? ` 等 ${result.failed} 条错误` : ""}`,
        );
      } else {
        setIoMessage(`导入完成：新建 ${result.created} 条，跳过 ${result.skipped} 条。`);
        void fetchStats(environmentMode, selectedSshConfigId, currentProjectId);
        void loadReport();
      }
    } catch (error) {
      setIoMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="space-y-6">
      <OnboardingChecklist />

      <div className="flex flex-wrap items-center justify-end gap-2">
        <input
          ref={importInputRef}
          type="file"
          accept="application/json,.json"
          className="hidden"
          onChange={(event) => void handleImportFile(event)}
        />
        <Button
          variant="outline"
          size="sm"
          onClick={handleImportClick}
          disabled={importing || exporting || exportingCsv}
        >
          {importing ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Upload className="h-4 w-4" />
          )}
          导入任务 JSON
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleExportJson()}
          disabled={exporting || exportingCsv || importing}
        >
          {exporting ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          导出任务 JSON
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleExportCsv()}
          disabled={exportingCsv || exporting || importing}
        >
          {exportingCsv ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          导出任务 CSV
        </Button>
        {ioMessage && <p className="text-[11px] text-muted-foreground">{ioMessage}</p>}
      </div>
      <DashboardStats />

      {reportError && (
        <Card className="flex flex-wrap items-center justify-between gap-3 border-destructive/40 p-4">
          <div className="space-y-1">
            <h3 className="text-sm font-semibold text-destructive">增强报表加载失败</h3>
            <p className="text-xs text-muted-foreground">{reportError}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={reportLoading}
            onClick={() => void loadReport()}
          >
            {reportLoading ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            重试
          </Button>
        </Card>
      )}

      {reportLoading && !report && !reportError && (
        <Card className="flex items-center gap-2 p-4 text-xs text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          正在加载增强报表…
        </Card>
      )}

      {report && (
        <Card className="p-4 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-semibold">增强报表</h3>
            <p className="text-xs text-muted-foreground">
              完成率 {report.completion_rate.toFixed(1)}% · 逾期 {report.overdue_tasks} · 阻塞{" "}
              {report.blocked_tasks} · 老化进行中 {report.aging_in_progress ?? 0}（超过{" "}
              {report.aging_days ?? 7} 天）
            </p>
          </div>
          <div className="grid gap-4 lg:grid-cols-2">
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">近 7 日完成趋势</p>
              <div className="flex h-28 items-end gap-2">
                {report.weekly_completed.map((point) => {
                  const height = maxDaily > 0 ? (point.count / maxDaily) * 100 : 0;
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
              <p className="mb-2 text-xs font-medium text-muted-foreground">近 8 周完成趋势</p>
              <div className="flex h-28 items-end gap-1.5">
                {(report.weekly_completed_series ?? []).map((point) => {
                  const height = maxWeeklySeries > 0 ? (point.count / maxWeeklySeries) * 100 : 0;
                  const shortLabel = point.label.includes("-W")
                    ? point.label.split("-W")[1]
                    : point.label;
                  return (
                    <div key={point.label} className="flex flex-1 flex-col items-center gap-1">
                      <span className="text-[10px] font-medium">{point.count}</span>
                      <div className="flex h-20 w-full items-end rounded-sm bg-muted/40 px-0.5">
                        <div
                          className="w-full rounded-t bg-emerald-500/80 transition-all"
                          style={{ height: point.count > 0 ? `${Math.max(height, 8)}%` : "0%" }}
                          title={point.label}
                        />
                      </div>
                      <span className="text-[10px] text-muted-foreground" title={point.label}>
                        W{shortLabel}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
            <div className="lg:col-span-2">
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
