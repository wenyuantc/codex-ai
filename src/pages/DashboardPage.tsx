import { useCallback, useEffect, useRef, useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";
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
  type DashboardTrendRange,
} from "@/lib/backend";
import {
  formatTokenCount,
  getDashboardTrendRangeOptions,
  normalizeTrendRange,
  shortTrendPointLabel,
  trendRangeChartTitle,
} from "@/lib/dashboardReport";
import { TASK_STATUSES } from "@/lib/types";
import { getStatusLabel, getStatusColor } from "@/lib/utils";

export function DashboardPage() {
  const { t } = useTranslation("dashboard");
  const { stats, fetchStats } = useDashboardStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const [report, setReport] = useState<DashboardReportSummary | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [reportError, setReportError] = useState<string | null>(null);
  const [trendRange, setTrendRange] = useState<DashboardTrendRange>("7d");
  const [milestoneId, setMilestoneId] = useState<string | null>(null);
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
  const appliedTrendRange = normalizeTrendRange(report?.trend_range ?? trendRange);
  const trendSeries = report?.trend_series?.length
    ? report.trend_series
    : (report?.weekly_completed ?? []);
  const maxTrend = Math.max(...trendSeries.map((p) => p.count), 0);
  const maxBurndown = Math.max(...(report?.milestone_burndown?.map((p) => p.remaining) ?? [0]), 0);
  const maxTokenTrend = Math.max(...(report?.token_usage_series?.map((p) => p.count) ?? [0]), 0);

  const loadReport = useCallback(async () => {
    setReportLoading(true);
    setReportError(null);
    try {
      const summary = await getDashboardReportSummary({
        projectId: currentProjectId,
        environmentMode,
        selectedSshConfigId,
        trendRange,
        milestoneId,
      });
      setReport(summary);
    } catch (error) {
      setReport(null);
      setReportError(error instanceof Error ? error.message : String(error));
    } finally {
      setReportLoading(false);
    }
  }, [currentProjectId, environmentMode, milestoneId, selectedSshConfigId, trendRange]);

  useEffect(() => {
    void fetchStats(environmentMode, selectedSshConfigId, currentProjectId);
  }, [currentProjectId, environmentMode, fetchStats, selectedSshConfigId]);

  useEffect(() => {
    void loadReport();
  }, [loadReport]);

  // Reset milestone selection when project/SSH scope changes so auto-pick can refresh.
  useEffect(() => {
    setMilestoneId(null);
  }, [currentProjectId, environmentMode, selectedSshConfigId]);

  const selectedMilestoneValue = milestoneId ?? report?.selected_milestone_id ?? "";

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
        t("io.exportedJson", {
          count: result.task_count,
          truncated: result.truncated ? t("io.exportedJsonTruncated") : "",
        }),
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
      setIoMessage(t("io.exportedCsv", { count: result.row_count }));
    } catch (error) {
      setIoMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingCsv(false);
    }
  };

  const handleImportClick = () => {
    if (!currentProjectId) {
      setIoMessage(t("io.selectProjectFirst"));
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
      setIoMessage(t("io.selectProjectFirst"));
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
        const firstError = result.errors[0]?.message ?? t("io.validationFailed");
        setIoMessage(
          t("io.importFailed", {
            error: firstError,
            more: result.failed > 1 ? t("io.importFailedMore", { count: result.failed }) : "",
          }),
        );
      } else {
        setIoMessage(t("io.importDone", { created: result.created, skipped: result.skipped }));
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
          {t("io.importJson")}
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
          {t("io.exportJson")}
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
          {t("io.exportCsv")}
        </Button>
        {ioMessage && <p className="text-[11px] text-muted-foreground">{ioMessage}</p>}
      </div>
      <DashboardStats />

      {reportError && (
        <Card className="flex flex-wrap items-center justify-between gap-3 border-destructive/40 p-4">
          <div className="space-y-1">
            <h3 className="text-sm font-semibold text-destructive">{t("report.loadFailed")}</h3>
            <p className="text-xs text-muted-foreground">{reportError}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={reportLoading}
            onClick={() => void loadReport()}
          >
            {reportLoading ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {t("report.retry")}
          </Button>
        </Card>
      )}

      {reportLoading && !report && !reportError && (
        <Card className="flex items-center gap-2 p-4 text-xs text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          {t("report.loading")}
        </Card>
      )}

      {report && (
        <Card className="p-4 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="space-y-1">
              <h3 className="text-sm font-semibold">{t("report.title")}</h3>
              <p className="text-xs text-muted-foreground">
                {t("report.summary", {
                  rate: report.completion_rate.toFixed(1),
                  overdue: report.overdue_tasks,
                  blocked: report.blocked_tasks,
                  aging: report.aging_in_progress ?? 0,
                  days: report.aging_days ?? 7,
                })}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex items-center gap-1 rounded-md border p-0.5">
                {getDashboardTrendRangeOptions().map((option) => (
                  <Button
                    key={option.value}
                    type="button"
                    size="sm"
                    variant={trendRange === option.value ? "default" : "ghost"}
                    className="h-7 px-2 text-xs"
                    disabled={reportLoading}
                    onClick={() => setTrendRange(option.value)}
                  >
                    {option.label}
                  </Button>
                ))}
              </div>
              <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
                {t("report.milestone")}
                <select
                  className="h-7 max-w-[12rem] rounded-md border bg-background px-2 text-xs text-foreground"
                  value={selectedMilestoneValue}
                  disabled={reportLoading || (report.milestones?.length ?? 0) === 0}
                  onChange={(event) => {
                    const next = event.target.value;
                    setMilestoneId(next.length > 0 ? next : null);
                  }}
                >
                  {(report.milestones?.length ?? 0) === 0 ? (
                    <option value="">{t("report.noMilestones")}</option>
                  ) : (
                    report.milestones.map((milestone) => (
                      <option key={milestone.id} value={milestone.id}>
                        {milestone.name}
                      </option>
                    ))
                  )}
                </select>
              </label>
              {reportLoading && (
                <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
              )}
            </div>
          </div>
          <div className="grid gap-4 lg:grid-cols-2">
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                {trendRangeChartTitle(appliedTrendRange)}
              </p>
              <div className="flex h-28 items-end gap-1.5">
                {trendSeries.map((point, index) => {
                  const height = maxTrend > 0 ? (point.count / maxTrend) * 100 : 0;
                  const shortLabel = shortTrendPointLabel(point.label, appliedTrendRange);
                  return (
                    <div
                      key={`${appliedTrendRange}-${point.label}-${index}`}
                      className="flex flex-1 flex-col items-center gap-1"
                    >
                      <span className="text-[10px] font-medium">{point.count}</span>
                      <div className="flex h-20 w-full items-end rounded-sm bg-muted/40 px-0.5">
                        <div
                          className="w-full rounded-t bg-primary/80 transition-all"
                          style={{ height: point.count > 0 ? `${Math.max(height, 8)}%` : "0%" }}
                          title={point.label}
                        />
                      </div>
                      <span className="text-[10px] text-muted-foreground" title={point.label}>
                        {shortLabel}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                {t("report.burndownTitle")}
              </p>
              {report.milestone_burndown_empty_reason ||
              (report.milestone_burndown?.length ?? 0) === 0 ? (
                <div className="flex h-28 items-center rounded-md border border-dashed px-3">
                  <p className="text-xs text-muted-foreground">
                    {report.milestone_burndown_empty_reason ?? t("report.burndownEmpty")}
                  </p>
                </div>
              ) : (
                <div className="flex h-28 items-end gap-1.5">
                  {report.milestone_burndown.map((point, index) => {
                    const height = maxBurndown > 0 ? (point.remaining / maxBurndown) * 100 : 0;
                    return (
                      <div
                        key={`${point.label}-${index}`}
                        className="flex flex-1 flex-col items-center gap-1"
                      >
                        <span className="text-[10px] font-medium">{point.remaining}</span>
                        <div className="flex h-20 w-full items-end rounded-sm bg-muted/40 px-0.5">
                          <div
                            className="w-full rounded-t bg-amber-500/80 transition-all"
                            style={{
                              height: point.remaining > 0 ? `${Math.max(height, 8)}%` : "0%",
                            }}
                            title={t("report.remainingTitle", {
                              label: point.label,
                              remaining: point.remaining,
                            })}
                          />
                        </div>
                        <span className="text-[10px] text-muted-foreground">{point.label}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
            <div className="lg:col-span-2">
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                {t("report.tokenTitle")}
              </p>
              {(report.token_usage?.sessions_with_usage ?? 0) === 0 ? (
                <div className="flex h-16 items-center rounded-md border border-dashed px-3">
                  <p className="text-xs text-muted-foreground">{t("report.tokenEmpty")}</p>
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                    <span>
                      {t("report.tokenSummary", {
                        total: formatTokenCount(report.token_usage.total_tokens),
                        input: formatTokenCount(report.token_usage.input_tokens),
                        output: formatTokenCount(report.token_usage.output_tokens),
                        withUsage: report.token_usage.sessions_with_usage,
                        sessions: report.token_usage.session_count,
                      })}
                    </span>
                    {report.token_usage_by_provider.map((item) => (
                      <span
                        key={item.provider}
                        className="rounded-full border px-2 py-0.5 font-mono text-[10px]"
                        title={t("report.tokenProviderTitle", {
                          provider: item.provider,
                          input: formatTokenCount(item.input_tokens),
                          output: formatTokenCount(item.output_tokens),
                        })}
                      >
                        {item.provider} {formatTokenCount(item.total_tokens)}
                      </span>
                    ))}
                  </div>
                  <div className="flex h-24 items-end gap-1.5">
                    {report.token_usage_series.map((point, index) => {
                      const height = maxTokenTrend > 0 ? (point.count / maxTokenTrend) * 100 : 0;
                      const shortLabel = shortTrendPointLabel(point.label, appliedTrendRange);
                      return (
                        <div
                          key={`token-${appliedTrendRange}-${point.label}-${index}`}
                          className="flex flex-1 flex-col items-center gap-1"
                        >
                          <span className="text-[10px] font-medium">
                            {formatTokenCount(point.count)}
                          </span>
                          <div className="flex h-16 w-full items-end rounded-sm bg-muted/40 px-0.5">
                            <div
                              className="w-full rounded-t bg-emerald-500/80 transition-all"
                              style={{
                                height: point.count > 0 ? `${Math.max(height, 8)}%` : "0%",
                              }}
                              title={`${point.label}: ${point.count.toLocaleString()}`}
                            />
                          </div>
                          <span className="text-[10px] text-muted-foreground" title={point.label}>
                            {shortLabel}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
            <div className="lg:col-span-2">
              <p className="mb-2 text-xs font-medium text-muted-foreground">
                {t("report.workloadTitle")}
              </p>
              <div className="max-h-28 space-y-1.5 overflow-auto">
                {report.employee_workload.length === 0 && (
                  <p className="text-xs text-muted-foreground">{t("report.noEmployees")}</p>
                )}
                {report.employee_workload.map((item) => (
                  <div
                    key={item.employee_id}
                    className="flex items-center justify-between gap-2 text-xs"
                  >
                    <span className="truncate font-medium">{item.employee_name}</span>
                    <span className="shrink-0 text-muted-foreground">
                      {t("report.workloadRow", {
                        active: item.active_tasks,
                        completed: item.completed_tasks,
                      })}
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
          <h3 className="text-sm font-semibold mb-3">{t("taskDistribution")}</h3>
          <div className="grid grid-cols-[repeat(auto-fit,minmax(4.75rem,1fr))] items-end gap-3">
            {taskDistribution.map(({ status, count }) => {
              const height = maxTaskCount > 0 ? (count / maxTaskCount) * 100 : 0;
              const label = getStatusLabel(status.value);

              return (
                <div
                  key={status.value}
                  className="flex min-w-0 flex-col items-center gap-1"
                  title={t("taskCountTitle", { label, count })}
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
