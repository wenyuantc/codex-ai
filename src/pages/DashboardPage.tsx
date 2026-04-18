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
          <div className="flex items-end gap-4 h-32">
            {TASK_STATUSES.map((status) => {
              const count = stats.tasksByStatus[status.value] ?? 0;
              const maxCount = Math.max(
                ...Object.values(stats.tasksByStatus),
                1
              );
              const height = (count / maxCount) * 100;
              return (
                <div key={status.value} className="flex-1 flex flex-col items-center gap-1">
                  <span className="text-sm font-bold">{count}</span>
                  <div className="w-full flex items-end justify-center" style={{ height: "80px" }}>
                    <div
                      className={`w-full max-w-[40px] rounded-t ${getStatusColor(status.value)} transition-all`}
                      style={{ height: `${Math.max(height, 4)}%` }}
                    />
                  </div>
                  <span className="text-[10px] text-muted-foreground">
                    {getStatusLabel(status.value)}
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
