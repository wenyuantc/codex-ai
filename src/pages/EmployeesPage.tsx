import { useState } from "react";
import { useLocation, useSearchParams } from "react-router-dom";
import { EmployeeList } from "@/components/employees/EmployeeList";
import { CreateEmployeeDialog } from "@/components/employees/CreateEmployeeDialog";
import { useProjectStore } from "@/stores/projectStore";
import { Plus } from "lucide-react";

export function EmployeesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const highlightedEmployeeId = searchParams.get("employeeId");
  const highlightedEmployeeNonce = (
    location.state as { globalSearchNonce?: number } | null
  )?.globalSearchNonce ?? null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">员工列表</h2>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          添加员工
        </button>
      </div>

      <EmployeeList
        projectId={currentProjectId}
        highlightedEmployeeId={highlightedEmployeeId}
        highlightedEmployeeNonce={highlightedEmployeeNonce}
      />
      <CreateEmployeeDialog
        open={showCreate}
        onOpenChange={setShowCreate}
        defaultProjectId={currentProjectId}
      />
    </div>
  );
}
