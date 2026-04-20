import { useDashboardStore } from "@/stores/dashboardStore";
import { BellRing, FolderKanban, ListTodo, TriangleAlert, TrendingUp, Users } from "lucide-react";
import { Card } from "@/components/ui/card";

export function DashboardStats() {
  const stats = useDashboardStore((state) => state.stats);

  const cards = [
    {
      icon: FolderKanban,
      label: "活跃项目",
      value: stats?.activeProjects ?? 0,
      color: "text-blue-500",
      bg: "bg-blue-500/10",
    },
    {
      icon: ListTodo,
      label: "总任务数",
      value: stats?.totalTasks ?? 0,
      color: "text-orange-500",
      bg: "bg-orange-500/10",
    },
    {
      icon: Users,
      label: "在线员工",
      value: `${stats?.onlineEmployees ?? 0} / ${stats?.totalEmployees ?? 0}`,
      color: "text-green-500",
      bg: "bg-green-500/10",
    },
    {
      icon: TrendingUp,
      label: "完成率",
      value: `${stats?.completionRate ?? 0}%`,
      color: "text-purple-500",
      bg: "bg-purple-500/10",
    },
    {
      icon: BellRing,
      label: "未读通知",
      value: stats?.unreadNotifications ?? 0,
      color: "text-sky-500",
      bg: "bg-sky-500/10",
    },
    {
      icon: TriangleAlert,
      label: "高优先级告警",
      value: stats?.highSeverityNotifications ?? 0,
      color: "text-rose-500",
      bg: "bg-rose-500/10",
    },
  ];

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-6 gap-4">
      {cards.map((card) => (
        <Card key={card.label} className="p-4 flex items-center gap-4">
          <div className={`${card.bg} p-3 rounded-lg`}>
            <card.icon className={`h-6 w-6 ${card.color}`} />
          </div>
          <div>
            <div className="text-2xl font-bold">{card.value}</div>
            <div className="text-xs text-muted-foreground">{card.label}</div>
          </div>
        </Card>
      ))}
    </div>
  );
}
