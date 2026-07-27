import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";

import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { KanbanBoard } from "@/components/tasks/KanbanBoard";
import { ArchiveManagementDialog } from "@/components/tasks/ArchiveManagementDialog";
import { CreateTaskDialog } from "@/components/tasks/CreateTaskDialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { batchUpdateTasks, listMilestones, listTags } from "@/lib/backend";
import type { Milestone, Tag } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Archive, CheckSquare, Plus } from "lucide-react";

export function KanbanPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { fetchTasks, tasks } = useTaskStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const projects = useProjectStore((state) => state.projects);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const { fetchEmployees } = useEmployeeStore();
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [showArchiveDialog, setShowArchiveDialog] = useState(false);
  const [overdueOnly, setOverdueOnly] = useState(false);
  const [blockedOnly, setBlockedOnly] = useState(false);
  const [milestoneFilter, setMilestoneFilter] = useState<string>("all");
  const [tagFilter, setTagFilter] = useState<string>("all");
  const [keyword, setKeyword] = useState("");
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [selectedTaskIds, setSelectedTaskIds] = useState<string[]>([]);
  const [batchStatus, setBatchStatus] = useState<string>("");
  const [batchMessage, setBatchMessage] = useState<string | null>(null);
  const [batchLoading, setBatchLoading] = useState(false);
  const visibleProjectIdsKey = projects.map((project) => project.id).join(",");
  const targetTaskId = searchParams.get("taskId");

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  useEffect(() => {
    void fetchTasks(currentProjectId);
  }, [currentProjectId, environmentMode, visibleProjectIdsKey, fetchTasks]);

  useEffect(() => {
    if (!currentProjectId) {
      setMilestones([]);
      setTags([]);
      return;
    }
    void listMilestones(currentProjectId).then(setMilestones).catch(() => setMilestones([]));
    void listTags(currentProjectId).then(setTags).catch(() => setTags([]));
  }, [currentProjectId]);

  const hasProjects = projects.length > 0;

  const filteredTaskIds = useMemo(() => {
    const normalized = keyword.trim().toLowerCase();
    return tasks
      .filter((task) => task.status !== "archived")
      .filter((task) => (overdueOnly ? Boolean(task.due_date) : true))
      .filter((task) => (blockedOnly ? task.status === "blocked" : true))
      .filter((task) =>
        milestoneFilter === "all" ? true : (task.milestone_id ?? "") === milestoneFilter,
      )
      .filter((task) =>
        !normalized
          ? true
          : task.title.toLowerCase().includes(normalized)
            || (task.description ?? "").toLowerCase().includes(normalized),
      )
      .map((task) => task.id);
  }, [tasks, overdueOnly, blockedOnly, milestoneFilter, keyword]);

  useHotkeys("n", () => setShowCreateDialog(true), { preventDefault: true });
  useHotkeys("a", () => setShowArchiveDialog(true), { preventDefault: true });

  const handleBatchUpdate = async () => {
    if (selectedTaskIds.length === 0 || !batchStatus) {
      setBatchMessage("请选择任务并指定目标状态。");
      return;
    }
    setBatchLoading(true);
    setBatchMessage(null);
    try {
      await batchUpdateTasks({ task_ids: selectedTaskIds, status: batchStatus });
      await fetchTasks(currentProjectId);
      setSelectedTaskIds([]);
      setBatchMessage(`已批量更新 ${selectedTaskIds.length} 个任务状态。`);
    } catch (error) {
      setBatchMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBatchLoading(false);
    }
  };

  return (
    <div className="h-full flex flex-col">
      <div className="mb-4 space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-lg font-semibold">看板列表</h2>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant={overdueOnly ? "default" : "outline"}
              onClick={() => setOverdueOnly((current) => !current)}
            >
              {overdueOnly ? "仅逾期" : "含未逾期"}
            </Button>
            <Button
              variant={blockedOnly ? "default" : "outline"}
              onClick={() => setBlockedOnly((current) => !current)}
            >
              {blockedOnly ? "仅阻塞" : "含未阻塞"}
            </Button>
            <Button onClick={() => setShowCreateDialog(true)}>
              <Plus className="h-4 w-4" />
              新建任务
              <Kbd variant="primary" size="xs" className="ml-1.5">N</Kbd>
            </Button>
            <Button variant="outline" onClick={() => setShowArchiveDialog(true)}>
              <Archive className="h-4 w-4" />
              归档管理
              <Kbd variant="subtle" size="xs" className="ml-1.5">A</Kbd>
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Input
            className="h-9 w-48"
            placeholder="搜索标题/描述"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <Select
            value={milestoneFilter}
            onValueChange={(value) => setMilestoneFilter(value ?? "all")}
          >
            <SelectTrigger className="h-9 w-40">
              <SelectValue placeholder="里程碑" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部里程碑</SelectItem>
              {milestones.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={tagFilter} onValueChange={(value) => setTagFilter(value ?? "all")}>
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder="标签" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部标签</SelectItem>
              {tags.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={batchStatus} onValueChange={(value) => setBatchStatus(value ?? "")}>
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder="批量改状态" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="todo">待办</SelectItem>
              <SelectItem value="in_progress">进行中</SelectItem>
              <SelectItem value="review">审核中</SelectItem>
              <SelectItem value="completed">已完成</SelectItem>
              <SelectItem value="blocked">阻塞</SelectItem>
              <SelectItem value="archived">归档</SelectItem>
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="sm"
            disabled={batchLoading}
            onClick={() => {
              setSelectedTaskIds(filteredTaskIds);
              setBatchMessage(`已选中当前筛选下 ${filteredTaskIds.length} 个任务。`);
            }}
          >
            <CheckSquare className="h-4 w-4" />
            全选筛选结果
          </Button>
          <Button size="sm" disabled={batchLoading} onClick={() => void handleBatchUpdate()}>
            批量更新 ({selectedTaskIds.length})
          </Button>
          {batchMessage && <span className="text-xs text-muted-foreground">{batchMessage}</span>}
        </div>
      </div>
      <div className="flex-1 overflow-hidden">
        {hasProjects ? (
          <KanbanBoard
            projectId={currentProjectId}
            overdueOnly={overdueOnly}
            blockedOnly={blockedOnly}
            milestoneId={milestoneFilter === "all" ? null : milestoneFilter}
            tagId={tagFilter === "all" ? null : tagFilter}
            keyword={keyword}
            selectedTaskIds={selectedTaskIds}
            onToggleTaskSelection={(taskId) => {
              setSelectedTaskIds((current) =>
                current.includes(taskId)
                  ? current.filter((id) => id !== taskId)
                  : [...current, taskId],
              );
            }}
            targetTaskId={targetTaskId}
            onClearTargetTask={() => {
              if (!targetTaskId) {
                return;
              }

              const nextSearchParams = new URLSearchParams(searchParams);
              nextSearchParams.delete("taskId");
              setSearchParams(nextSearchParams, { replace: true });
            }}
          />
        ) : (
          <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-border text-sm text-muted-foreground">
            {environmentMode === "ssh"
              ? "当前 SSH 视图还没有项目，请先到项目管理创建 SSH 项目。"
              : "当前没有可展示的本地项目。"}
          </div>
        )}
      </div>
      {showCreateDialog && (
        <CreateTaskDialog
          open={showCreateDialog}
          onOpenChange={setShowCreateDialog}
          projectId={currentProjectId}
        />
      )}
      {showArchiveDialog && (
        <ArchiveManagementDialog
          open={showArchiveDialog}
          onOpenChange={setShowArchiveDialog}
          defaultProjectId={currentProjectId}
        />
      )}
    </div>
  );
}
