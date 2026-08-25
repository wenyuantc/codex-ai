import type { Subtask, TaskAttachment, TaskFileRef } from "@/lib/types";
import { isImageAttachment } from "@/lib/taskAttachments";

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
  planContent?: string | null;
  subtasks?: Subtask[];
  attachments?: TaskAttachment[];
  fileRefs?: Array<Pick<TaskFileRef, "relative_path"> | string>;
  followUpPrompt?: string;
}

export interface TaskExecutionInput {
  prompt: string;
  imagePaths: string[];
}

export function buildTaskExecutionInput({
  title,
  description,
  planContent,
  subtasks = [],
  attachments = [],
  fileRefs = [],
  followUpPrompt,
}: BuildTaskExecutionPromptOptions): TaskExecutionInput {
  const trimmedPlan = planContent?.trim();
  const sections = trimmedPlan ? [`执行计划:\n${trimmedPlan}`] : [`任务标题:\n${title.trim()}`];
  const trimmedDescription = description?.trim();
  const trimmedFollowUpPrompt = followUpPrompt?.trim();
  const validAttachments = attachments.filter(
    (attachment) => attachment.stored_path.trim().length > 0,
  );

  if (!trimmedPlan && trimmedDescription) {
    sections.push(`任务描述:\n${trimmedDescription}`);
  }

  if (!trimmedPlan && subtasks.length > 0) {
    const subtaskLines = subtasks.map((subtask, index) => {
      const statusLabel = SUBTASK_STATUS_LABELS[subtask.status] ?? subtask.status;
      return `${index + 1}. [${statusLabel}] ${subtask.title}`;
    });
    sections.push(`子任务:\n${subtaskLines.join("\n")}`);
  }

  if (validAttachments.length > 0) {
    const attachmentLines = validAttachments.map(
      (attachment, index) => `${index + 1}. ${attachment.original_name}`,
    );
    sections.push(
      `任务附件:\n${attachmentLines.join("\n")}\n\n说明：以上附件已绑定到当前任务；其中图片会随本次任务一并附带给 Codex。`,
    );
  }

  const fileRefPaths = fileRefs
    .map((item) => (typeof item === "string" ? item : item.relative_path).trim())
    .filter((path) => path.length > 0);
  if (fileRefPaths.length > 0) {
    const fileRefLines = fileRefPaths.map((path, index) => `${index + 1}. ${path}`);
    sections.push(
      `项目文件引用:\n${fileRefLines.join("\n")}\n\n说明：以上路径相对项目仓库根目录。请优先阅读这些文件再动手。`,
    );
  }

  if (trimmedFollowUpPrompt) {
    sections.push(`本次补充指令:\n${trimmedFollowUpPrompt}`);
  }

  return {
    prompt: sections.join("\n\n"),
    imagePaths: validAttachments
      .filter((attachment) => isImageAttachment(attachment.stored_path, attachment.mime_type))
      .map((attachment) => attachment.stored_path),
  };
}

export function buildTaskExecutionPrompt(options: BuildTaskExecutionPromptOptions): string {
  return buildTaskExecutionInput(options).prompt;
}
