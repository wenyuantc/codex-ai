import { listEmployees } from "./backend";

// Suggest the best assignee for a task
export async function suggestAssignee(
  taskDescription: string,
): Promise<{ employeeId: string; reason: string }> {
  const allEmployees = await listEmployees();
  const employees = allEmployees.filter((employee) => employee.status !== "error");

  const developers = employees.filter((e) => e.role === "developer");
  const reviewers = employees.filter((e) => e.role === "reviewer");
  const testers = employees.filter((e) => e.role === "tester");
  const coordinators = employees.filter((e) => e.role === "coordinator");

  const desc = taskDescription.toLowerCase();
  const suggestions: { employeeId: string; reason: string; score: number }[] = [];

  if (desc.includes("测试") || desc.includes("test") || desc.includes("bug")) {
    testers.forEach((e) =>
      suggestions.push({ employeeId: e.id, reason: "测试相关任务", score: 10 }),
    );
  }
  if (desc.includes("审查") || desc.includes("review") || desc.includes("代码审查")) {
    reviewers.forEach((e) =>
      suggestions.push({ employeeId: e.id, reason: "审查相关任务", score: 10 }),
    );
  }
  if (desc.includes("协调") || desc.includes("计划") || desc.includes("schedule")) {
    coordinators.forEach((e) =>
      suggestions.push({ employeeId: e.id, reason: "协调相关任务", score: 10 }),
    );
  }
  developers.forEach((e) =>
    suggestions.push({ employeeId: e.id, reason: "开发相关任务", score: 5 }),
  );

  suggestions.sort((a, b) => b.score - a.score);
  if (suggestions.length > 0) {
    return { employeeId: suggestions[0].employeeId, reason: suggestions[0].reason };
  }
  if (employees.length > 0) {
    return { employeeId: employees[0].id, reason: "默认推荐" };
  }
  return { employeeId: "", reason: "无可用员工" };
}

// Analyze task complexity (1-10)
export async function analyzeComplexity(
  taskDescription: string,
): Promise<{ score: number; breakdown: string }> {
  const desc = taskDescription.toLowerCase();
  let score = 3;

  if (desc.includes("架构") || desc.includes("重构") || desc.includes("architect")) score += 3;
  if (desc.includes("集成") || desc.includes("integration")) score += 2;
  if (desc.includes("安全") || desc.includes("security")) score += 2;
  if (desc.includes("性能") || desc.includes("performance")) score += 1;
  if (desc.includes("测试") || desc.includes("test")) score += 1;
  if (desc.length > 200) score += 1;
  if (desc.length > 500) score += 1;

  score = Math.min(score, 10);

  const breakdown = `基于任务描述分析：${score <= 3 ? "简单任务" : score <= 6 ? "中等复杂度" : score <= 8 ? "高复杂度" : "极高复杂度"}。影响因素：${desc.length > 200 ? "描述较长" : "描述简短"}，${desc.includes("架构") || desc.includes("重构") ? "涉及架构设计" : "常规开发"}，${desc.includes("集成") ? "需要集成工作" : "独立实现"}`;

  return { score, breakdown };
}

// Generate an AI comment for a task
export async function generateAIComment(
  taskTitle: string,
  taskDescription: string,
  taskStatus: string,
): Promise<string> {
  const statusComments: Record<string, string> = {
    todo: "任务待开始，建议先评估复杂度和所需资源。",
    in_progress: "任务进行中，请定期更新进度。",
    review: "任务已进入审核阶段，请安排审查。",
    completed: "任务已完成，建议进行回顾总结。",
    blocked: "任务被阻塞，建议排查阻塞原因并寻求支持。",
  };

  const baseComment = statusComments[taskStatus] ?? "任务状态更新。";
  return `[AI 助手] ${baseComment} 任务"${taskTitle}"当前状态：${taskStatus}。${taskDescription ? `描述概要：${taskDescription.slice(0, 100)}${taskDescription.length > 100 ? "..." : ""}` : ""}`;
}
