import { useEffect, useState } from "react";
import { useEmployeeStore } from "@/stores/employeeStore";
import { EmployeeCard } from "./EmployeeCard";

interface EmployeeListProps {
  projectId?: string;
  highlightedEmployeeId?: string | null;
  highlightedEmployeeNonce?: number | null;
}

export function EmployeeList({
  projectId,
  highlightedEmployeeId,
  highlightedEmployeeNonce,
}: EmployeeListProps) {
  const { employees, fetchEmployees } = useEmployeeStore();
  const [filter, setFilter] = useState<string>("all");

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  const projectEmployees = projectId
    ? employees.filter((employee) => employee.project_id === projectId)
    : employees;
  const filtered = filter === "all"
    ? projectEmployees
    : projectEmployees.filter((employee) => employee.status === filter);

  useEffect(() => {
    if (!highlightedEmployeeId || filter === "all") {
      return;
    }

    const highlightedEmployee = projectEmployees.find((employee) => employee.id === highlightedEmployeeId);
    if (!highlightedEmployee) {
      return;
    }

    if (highlightedEmployee.status !== filter) {
      setFilter("all");
    }
  }, [filter, highlightedEmployeeId, highlightedEmployeeNonce, projectEmployees]);

  useEffect(() => {
    if (!highlightedEmployeeId) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      document
        .getElementById(`employee-card-${highlightedEmployeeId}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);

    return () => window.clearTimeout(timeoutId);
  }, [filtered.length, highlightedEmployeeId, highlightedEmployeeNonce]);

  return (
    <div className="space-y-3">
      {/* Filter */}
      <div className="flex items-center gap-2">
        {["all", "online", "busy", "offline", "error"].map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-2.5 py-1 text-xs rounded-md transition-colors ${
              filter === f
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent"
            }`}
          >
            {f === "all" ? "全部" : f === "online" ? "在线" : f === "busy" ? "忙碌" : f === "offline" ? "离线" : "错误"}
          </button>
        ))}
        <span className="text-xs text-muted-foreground ml-auto">
          {filtered.length} 名员工
        </span>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filtered.map((emp) => (
          <EmployeeCard
            key={emp.id}
            employee={emp}
            highlighted={emp.id === highlightedEmployeeId}
          />
        ))}
      </div>

      {filtered.length === 0 && (
        <div className="text-center py-12 text-muted-foreground text-sm">
          {filter === "all" ? "暂无员工" : `没有${filter === "online" ? "在线" : filter === "busy" ? "忙碌" : filter === "offline" ? "离线" : "错误"}员工`}
        </div>
      )}
    </div>
  );
}
