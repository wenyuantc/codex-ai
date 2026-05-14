import { useEffect, useCallback, useState } from "react";
import { useDashboardStore } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Activity, RefreshCw } from "lucide-react";
import { ActivityListDialog } from "./ActivityListDialog";
import { ActivityLogItem } from "./ActivityLogItem";

export function ActivityFeed() {
  const { recentActivities, fetchRecentActivities } = useDashboardStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const currentProjectName = useProjectStore((state) => state.currentProject?.name);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const [dialogOpen, setDialogOpen] = useState(false);

  useEffect(() => {
    void fetchRecentActivities(environmentMode, selectedSshConfigId, 30, currentProjectId);
  }, [currentProjectId, environmentMode, fetchRecentActivities, selectedSshConfigId]);

  const refresh = useCallback(() => {
    void fetchRecentActivities(environmentMode, selectedSshConfigId, 30, currentProjectId);
  }, [currentProjectId, environmentMode, fetchRecentActivities, selectedSshConfigId]);

  // Auto-refresh every 30 seconds
  useEffect(() => {
    const interval = setInterval(refresh, 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  return (
    <Card className="p-4 flex flex-col h-full">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Activity className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-sm font-semibold">最近活动</h3>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={() => setDialogOpen(true)}>
            查看更多
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={refresh}
            title="刷新最近活动"
            aria-label="刷新最近活动"
          >
            <RefreshCw className="h-3.5 w-3.5 text-muted-foreground" />
          </Button>
        </div>
      </div>

      {recentActivities.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-8">
          暂无活动记录
        </div>
      ) : (
        <ScrollArea className="flex-1 max-h-[400px]">
          <div className="space-y-2 pr-3">
            {recentActivities.map((activity) => (
              <ActivityLogItem key={activity.id} activity={activity} />
            ))}
          </div>
        </ScrollArea>
      )}

      <ActivityListDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        projectId={currentProjectId}
        projectName={currentProjectName}
        environmentMode={environmentMode}
        selectedSshConfigId={selectedSshConfigId}
      />
    </Card>
  );
}
