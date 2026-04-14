import { chdir } from "node:process";
import { stdin, stdout, stderr, exit } from "node:process";
import { Codex } from "@openai/codex-sdk";

function extractText(result) {
  if (typeof result === "string") {
    return result;
  }

  if (!result || typeof result !== "object") {
    return String(result ?? "");
  }

  const directCandidates = [
    result.finalResponse,
    result.final_response,
    result.outputText,
    result.output_text,
    result.response,
    result.text,
    result.content,
  ];

  for (const candidate of directCandidates) {
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate;
    }
  }

  if (Array.isArray(result.messages)) {
    const text = result.messages
      .flatMap((message) => {
        if (typeof message === "string") {
          return [message];
        }
        if (!message || typeof message !== "object") {
          return [];
        }
        return [
          message.text,
          message.content,
          message.output_text,
          message.outputText,
        ].filter((value) => typeof value === "string" && value.trim());
      })
      .join("\n")
      .trim();

    if (text) {
      return text;
    }
  }

  return JSON.stringify(result);
}

function emit(line) {
  if (line && String(line).trim()) {
    stdout.write(`${String(line).trimEnd()}\n`);
  }
}

function emitError(line) {
  if (line && String(line).trim()) {
    stderr.write(`${String(line).trimEnd()}\n`);
  }
}

function emitMultiline(text) {
  String(text)
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0)
    .forEach((line) => emit(line));
}

async function readInput() {
  let raw = "";
  for await (const chunk of stdin) {
    raw += chunk.toString();
  }

  if (!raw.trim()) {
    throw new Error("missing input payload");
  }

  return JSON.parse(raw);
}

function summarizeFileChange(item) {
  const changes = Array.isArray(item.changes) ? item.changes : [];
  if (changes.length === 0) {
    return null;
  }

  const files = changes
    .map((change) => `${change.kind}:${change.path}`)
    .slice(0, 5)
    .join(", ");
  return `[文件变更] ${files}${changes.length > 5 ? " ..." : ""}`;
}

function emitItemStarted(item) {
  switch (item?.type) {
    case "command_execution":
      emit(`[命令] ${item.command}`);
      break;
    case "mcp_tool_call":
      emit(`[MCP] ${item.server}/${item.tool}`);
      break;
    case "web_search":
      emit(`[搜索] ${item.query}`);
      break;
    default:
      break;
  }
}

function emitItemUpdate(item, state) {
  if (!item || typeof item !== "object") {
    return;
  }

  switch (item.type) {
    case "command_execution": {
      const previous = state.commandOutputs.get(item.id) ?? "";
      const next = typeof item.aggregated_output === "string" ? item.aggregated_output : "";
      const delta = next.startsWith(previous) ? next.slice(previous.length) : next;
      state.commandOutputs.set(item.id, next);
      if (delta.trim()) {
        emitMultiline(delta);
      }
      if (item.status === "failed" && typeof item.exit_code === "number") {
        emitError(`[ERROR] 命令退出，code=${item.exit_code}`);
      }
      break;
    }
    case "agent_message":
      if (typeof item.text === "string" && item.text.trim()) {
        const previous = state.agentMessages.get(item.id) ?? "";
        const next = item.text;
        const delta = next.startsWith(previous) ? next.slice(previous.length) : next;
        state.agentMessages.set(item.id, next);
        if (delta.trim()) {
          emitMultiline(delta);
        }
      }
      break;
    case "reasoning":
      if (typeof item.text === "string" && item.text.trim()) {
        const previous = state.reasoningMessages.get(item.id) ?? "";
        if (!previous) {
          emit(`[思考] ${item.text.trim()}`);
        }
        state.reasoningMessages.set(item.id, item.text);
      }
      break;
    case "file_change": {
      const summary = summarizeFileChange(item);
      if (summary) {
        emit(summary);
      }
      break;
    }
    case "error":
      emitError(`[ERROR] ${item.message}`);
      break;
    case "todo_list":
      if (Array.isArray(item.items) && item.items.length > 0) {
        const summary = item.items
          .map((entry) => `${entry.completed ? "[x]" : "[ ]"} ${entry.text}`)
          .join(" | ");
        if (summary !== state.lastTodoSummary) {
          state.lastTodoSummary = summary;
          emit(`[计划] ${summary}`);
        }
      }
      break;
    default:
      break;
  }
}

function normalizeInput(payload) {
  if (Array.isArray(payload.input) && payload.input.length > 0) {
    return payload.input;
  }

  const prompt = String(payload.prompt ?? "").trim();
  if (!prompt) {
    throw new Error("prompt or input is required");
  }

  return prompt;
}

async function runSession(thread, input) {
  const stream = await thread.runStreamed(input);
  const state = {
    commandOutputs: new Map(),
    agentMessages: new Map(),
    reasoningMessages: new Map(),
    lastTodoSummary: "",
    sessionId: null,
  };

  emit("[SDK] 任务已提交，等待 Codex 响应...");

  for await (const event of stream.events) {
    switch (event?.type) {
      case "thread.started":
        if (event.thread_id && event.thread_id !== state.sessionId) {
          state.sessionId = event.thread_id;
          emit(`session id: ${event.thread_id}`);
        }
        break;
      case "turn.started":
        emit("[SDK] 已开始执行");
        break;
      case "item.started":
        emitItemStarted(event.item);
        emitItemUpdate(event.item, state);
        break;
      case "item.updated":
      case "item.completed":
        emitItemUpdate(event.item, state);
        break;
      case "turn.completed":
        emit("[SDK] 执行完成");
        break;
      case "turn.failed":
        emitError(`[ERROR] ${event.error?.message ?? "任务执行失败"}`);
        break;
      case "error":
        emitError(`[ERROR] ${event.message ?? "SDK 流式事件失败"}`);
        break;
      default:
        break;
    }
  }
}

async function main() {
  let mode = "one_shot";
  try {
    const payload = await readInput();
    mode = payload.mode === "session" ? "session" : "one_shot";
    const input = normalizeInput(payload);

    const codex = new Codex();
    const threadOptions = {
      sandboxMode: "danger-full-access",
      skipGitRepoCheck: true,
    };
    if (typeof payload.model === "string" && payload.model.trim()) {
      threadOptions.model = payload.model.trim();
    }
    if (typeof payload.modelReasoningEffort === "string" && payload.modelReasoningEffort.trim()) {
      threadOptions.modelReasoningEffort = payload.modelReasoningEffort.trim();
    }
    if (typeof payload.workingDirectory === "string" && payload.workingDirectory.trim()) {
      const workingDirectory = payload.workingDirectory.trim();
      chdir(workingDirectory);
      threadOptions.workingDirectory = workingDirectory;
    }

    const thread =
      typeof payload.resumeSessionId === "string" && payload.resumeSessionId.trim()
        ? codex.resumeThread(payload.resumeSessionId.trim(), threadOptions)
        : codex.startThread(threadOptions);

    if (mode === "session") {
      await runSession(thread, input);
      return;
    }

    const result = await thread.run(input);
    const text = extractText(result).trim();
    stdout.write(
      JSON.stringify({
        ok: true,
        text,
      }),
    );
  } catch (error) {
    emitError(String(error?.stack || error?.message || error));
    if (mode !== "session" && String(error?.message || error).trim()) {
      stdout.write(
        JSON.stringify({
          ok: false,
          error: String(error?.message || error),
        }),
      );
    }
    exit(1);
  }
}

void main();
