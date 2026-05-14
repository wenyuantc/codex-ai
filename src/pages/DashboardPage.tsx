import { useEffect } from "react";
import { useDashboardStore } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { DashboardStats } from "@/components/dashboard/DashboardStats";
import { ActivityFeed } from "@/components/dashboard/ActivityFeed";
import { EmployeePerformanceChart } from "@/components/dashboard/EmployeePerformanceChart";
import { Card } from "@/components/ui/card";
import { TASK_STATUSES } from "@/lib/types";
import { getStatusLabel, getStatusColor } from "@/lib/utils";

export function DashboardPage() {
  const { stats, fetchStats } = useDashboardStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const taskDistribution = stats?.tasksByStatus
    ? TASK_STATUSES.map((status) => ({
        status,
        count: stats.tasksByStatus[status.value] ?? 0,
      }))
    : [];
  const maxTaskCount = Math.max(...taskDistribution.map((item) => item.count), 0);

  useEffect(() => {
    void fetchStats(environmentMode, selectedSshConfigId, currentProjectId);
  }, [currentProjectId, environmentMode, fetchStats, selectedSshConfigId]);

  return (
    <div className="space-y-6">
      <DashboardStats />

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
