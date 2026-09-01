export type SessionTodoStatus = "pending" | "in_progress" | "completed" | "cancelled";

export interface SessionTodoItem {
  content: string;
  status: SessionTodoStatus;
  priority?: string;
}

const TODO_TAG = "[待办]";
const EMPTY_SNAPSHOT = "[待办] (空)";
const ITEM_LINE = /^- \[([^\]]+)\] (.+)$/;
const PRIORITY_SUFFIX = /^(.*) \(([^)]+)\)$/;
const KNOWN_PRIORITY = /^(high|medium|low)$/i;

function normalizeStatus(raw: string): SessionTodoStatus {
  const status = raw.trim().toLowerCase().replace(/-/g, "_");
  if (status === "completed" || status === "complete" || status === "done") {
    return "completed";
  }
  if (status === "in_progress" || status === "doing") {
    return "in_progress";
  }
  if (status === "cancelled" || status === "canceled") {
    return "cancelled";
  }
  return "pending";
}

function isNonListTodoLabel(line: string): boolean {
  const trimmed = line.trim();
  return trimmed === "[待办] 读取任务清单" || trimmed === "[待办] 更新任务清单";
}

export function parseSessionTodoItemLine(line: string): SessionTodoItem | null {
  const match = line.trim().match(ITEM_LINE);
  if (!match) {
    return null;
  }
  let content = match[2].trim();
  let priority: string | undefined;
  const suffix = content.match(PRIORITY_SUFFIX);
  if (suffix && KNOWN_PRIORITY.test(suffix[2].trim())) {
    content = suffix[1].trim();
    priority = suffix[2].trim().toLowerCase();
  }
  if (!content) {
    return null;
  }
  return {
    content,
    status: normalizeStatus(match[1]),
    priority,
  };
}

export function parseSessionTodoSnapshot(text: string): SessionTodoItem[] | undefined {
  const trimmed = text.trim();
  if (!trimmed.startsWith(TODO_TAG)) {
    return undefined;
  }
  if (isNonListTodoLabel(trimmed)) {
    return undefined;
  }
  if (trimmed === EMPTY_SNAPSHOT) {
    return [];
  }

  const lines = trimmed.split(/\r?\n/);
  if (lines.length === 1) {
    return undefined;
  }

  const header = lines[0]?.trim() ?? "";
  if (header !== TODO_TAG) {
    return undefined;
  }

  return lines
    .slice(1)
    .map(parseSessionTodoItemLine)
    .filter((item): item is SessionTodoItem => Boolean(item));
}

function collectSplitTodoItems(lines: string[], startIndex: number): SessionTodoItem[] {
  const items: SessionTodoItem[] = [];
  for (let index = startIndex; index < lines.length; index += 1) {
    const item = parseSessionTodoItemLine(lines[index] ?? "");
    if (!item) {
      break;
    }
    items.push(item);
  }
  return items;
}

/**
 * Latest native `TodoWrite` snapshot in terminal log lines.
 * `undefined` means no snapshot; `[]` means the list was cleared.
 */
export function extractLatestSessionTodos(lines: string[]): SessionTodoItem[] | undefined {
  let latest: SessionTodoItem[] | undefined;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const snapshot = parseSessionTodoSnapshot(line);
    if (snapshot !== undefined) {
      latest = snapshot;
      continue;
    }
    if (line.trim() !== TODO_TAG) {
      continue;
    }
    const items = collectSplitTodoItems(lines, index + 1);
    if (items.length === 0) {
      continue;
    }
    latest = items;
    index += items.length;
  }
  return latest;
}

export function applyTodoSnapshotToKeys(
  current: Record<string, SessionTodoItem[]>,
  keys: string[],
  snapshot: SessionTodoItem[] | undefined,
): Record<string, SessionTodoItem[]> {
  if (snapshot === undefined || keys.length === 0) {
    return current;
  }
  const next = { ...current };
  for (const key of keys) {
    next[key] = snapshot;
  }
  return next;
}
