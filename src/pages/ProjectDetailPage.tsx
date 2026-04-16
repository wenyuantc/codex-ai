import { useEffect, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import type { Employee } from "@/lib/types";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { ArrowLeft, Edit2, Trash2 } from "lucide-react";
import { getStatusLabel, getStatusColor, getPriorityLabel, formatDate } from "@/lib/utils";
import { getProjectWorkingDir, getProjectTypeLabel } from "@/lib/projects";

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { projects, deleteProject } = useProjectStore();
  const { tasks, fetchTasks } = useTaskStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const [projectEmployees, setProjectEmployees] = useState<Employee[]>([]);
  const [showEdit, setShowEdit] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const project = projects.find((p) => p.id === id);

  useEffect(() => {
    if (id) {
      fetchTasks(id);
      fetchEmployees();
    }
  }, [id, fetchTasks, fetchEmployees]);

  useEffect(() => {
    if (!id) {
      setProjectEmployees([]);
      return;
    }

    setProjectEmployees(employees.filter((employee) => employee.project_id === id));
  }, [employees, id]);

  if (!project) {
    return (
      <div className="text-center py-12">
        <p className="text-muted-foreground mb-4">项目不存在</p>
        <Link to="/projects" className="text-primary hover:underline">
          返回项目列表
        </Link>
      </div>
    );
  }

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deleteProject(project.id);
      setShowDeleteConfirm(false);
      navigate("/projects");
    } finally {
      setDeleting(false);
    }
  };

  const tasksByStatus = {
    todo: tasks.filter((t) => t.status === "todo"),
    in_progress: tasks.filter((t) => t.status === "in_progress"),
    review: tasks.filter((t) => t.status === "review"),
    completed: tasks.filter((t) => t.status === "completed"),
    blocked: tasks.filter((t) => t.status === "blocked"),
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Link to="/projects" className="p-1 hover:bg-accent rounded">
          <ArrowLeft className="h-5 w-5" />
        </Link>
        <div className="flex-1">
          <h2 className="text-xl font-bold">{project.name}</h2>
          <div className="flex items-center gap-2 mt-1">
            <Badge variant={project.status === "active" ? "default" : "secondary"}>
              {getStatusLabel(project.status)}
            </Badge>
            <Badge variant="outline">{getProjectTypeLabel(project.project_type)}</Badge>
            <span className="text-xs text-muted-foreground">
              创建于 {formatDate(project.created_at)}
            </span>
          </div>
        </div>
        <Button variant="outline" size="sm" onClick={() => setShowEdit(true)}>
          <Edit2 className="h-3.5 w-3.5 mr-1" />
          编辑
        </Button>
        <Button variant="destructive" size="sm" onClick={() => setShowDeleteConfirm(true)}>
          <Trash2 className="h-3.5 w-3.5 mr-1" />
          删除
        </Button>
      </div>

      {/* Description */}
      {project.description && (
        <Card className="p-4">
          <p className="text-sm text-muted-foreground">{project.description}</p>
        </Card>
      )}

      <Card className="p-4">
        <h3 className="mb-3 text-sm font-semibold">仓库信息</h3>
        <RepoPathDisplay
          repoPath={getProjectWorkingDir(project)}
          projectType={project.project_type}
          showCopyAction
        />
      </Card>

      {/* Task Stats */}
      <div className="grid grid-cols-5 gap-3">
        {Object.entries(tasksByStatus).map(([status, items]) => (
          <Card key={status} className="p-3 text-center">
            <div className={`w-2 h-2 rounded-full mx-auto mb-1 ${getStatusColor(status)}`} />
            <div className="text-lg font-bold">{items.length}</div>
            <div className="text-xs text-muted-foreground">{getStatusLabel(status)}</div>
          </Card>
        ))}
      </div>

      {/* Task List */}
      <Card className="p-4">
        <h3 className="text-sm font-semibold mb-3">任务列表</h3>
        {tasks.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">暂无任务</p>
        ) : (
          <div className="space-y-2">
            {tasks.map((task) => (
              <div
                key={task.id}
                className="flex items-center gap-3 p-2 rounded-md hover:bg-accent/50 text-sm"
              >
                <div className={`w-1.5 h-1.5 rounded-full ${getStatusColor(task.status)}`} />
                <span className="flex-1 font-medium truncate">{task.title}</span>
                <span className="text-xs text-muted-foreground">
                  {getPriorityLabel(task.priority)}
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* Team Members */}
      <Card className="p-4">
        <h3 className="text-sm font-semibold mb-3">团队成员</h3>
        {projectEmployees.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">暂无成员</p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {projectEmployees.map((emp) => (
              <div
                key={emp.id}
                className="flex items-center gap-2 px-3 py-1.5 bg-secondary rounded-full text-sm"
              >
                <div className={`w-2 h-2 rounded-full ${getStatusColor(emp.status)}`} />
                <span>{emp.name}</span>
                <span className="text-xs text-muted-foreground">{emp.role}</span>
              </div>
            ))}
          </div>
        )}
      </Card>

      <EditProjectDialog open={showEdit} onOpenChange={setShowEdit} project={project} />

      <DeleteProjectDialog
        open={showDeleteConfirm}
        onOpenChange={(open) => {
          if (!open && !deleting) setShowDeleteConfirm(false);
        }}
        project={project}
        deleting={deleting}
        onConfirm={handleDelete}
      />
    </div>
  );
}
