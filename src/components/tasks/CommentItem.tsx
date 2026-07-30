import { useEffect, useState } from "react";
import type { Comment } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Badge } from "@/components/ui/badge";
import { formatDate } from "@/lib/utils";

interface CommentItemProps {
  comment: Comment;
}

export function CommentItem({ comment }: CommentItemProps) {
  const { employees, fetchEmployees } = useEmployeeStore();
  const [authorName, setAuthorName] = useState<string>("人类");

  useEffect(() => {
    if (employees.length === 0) {
      fetchEmployees();
    }
  }, [employees.length, fetchEmployees]);

  useEffect(() => {
    if (comment.employee_id) {
      const emp = employees.find((e) => e.id === comment.employee_id);
      if (emp) setAuthorName(emp.name);
    }
  }, [comment.employee_id, employees]);

  return (
    <div className="flex gap-2">
      <div className="shrink-0 w-6 h-6 rounded-full bg-primary/10 flex items-center justify-center text-primary text-[10px] font-medium">
        {authorName[0]}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-xs font-medium">{authorName}</span>
          {comment.is_ai_generated === 1 && (
            <Badge variant="secondary" className="text-[9px] px-1 py-0 h-4">
              AI
            </Badge>
          )}
          <span className="text-[10px] text-muted-foreground">
            {formatDate(comment.created_at)}
          </span>
        </div>
        <p className="text-xs text-foreground mt-0.5 whitespace-pre-wrap">{comment.content}</p>
      </div>
    </div>
  );
}
