import { useEffect, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Activity, RefreshCw } from "lucide-react";

import { useDashboardStore } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ActivityListDialog } from "./ActivityListDialog";
import { ActivityLogItem } from "./ActivityLogItem";

export function ActivityFeed() {
  const { t } = useTranslation("dashboard");
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
          <h3 className="text-sm font-semibold">{t("recentActivity")}</h3>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={() => setDialogOpen(true)}>
            {t("viewMore")}
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={refresh}
            title={t("refreshActivity")}
            aria-label={t("refreshActivity")}
          >
            <RefreshCw className="h-3.5 w-3.5 text-muted-foreground" />
          </Button>
        </div>
      </div>

      {recentActivities.length === 0 ? (
        <div className="text-sm text-muted-foreground text-center py-8">
          {t("noActivityRecords")}
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
