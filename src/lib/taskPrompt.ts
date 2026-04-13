import type { Subtask } from "@/lib/types";

const SUBTASK_STATUS_LABELS: Record<string, string> = {
  todo: "待办",
  in_progress: "进行中",
  review: "审核中",
  completed: "已完成",
  blocked: "已阻塞",
};

interface BuildTaskExecutionPromptOptions {
  title: string;
  description?: string | null;
  subtasks?: Subtask[];
  followUpPrompt?: string;
}

export function buildTaskExecutionPrompt({
  title,
  description,
  subtasks = [],
  followUpPrompt,
}: BuildTaskExecutionPromptOptions): string {
  const sections = [`任务标题:\n${title.trim()}`];
  const trimmedDescription = description?.trim();
  const trimmedFollowUpPrompt = followUpPrompt?.trim();

  if (trimmedDescription) {
    sections.push(`任务描述:\n${trimmedDescription}`);
  }

  if (subtasks.length > 0) {
    const subtaskLines = subtasks.map((subtask, index) => {
      const statusLabel = SUBTASK_STATUS_LABELS[subtask.status] ?? subtask.status;
      return `${index + 1}. [${statusLabel}] ${subtask.title}`;
    });
    sections.push(`子任务:\n${subtaskLines.join("\n")}`);
  }

  if (trimmedFollowUpPrompt) {
    sections.push(`本次补充指令:\n${trimmedFollowUpPrompt}`);
  }

  return sections.join("\n\n");
}
