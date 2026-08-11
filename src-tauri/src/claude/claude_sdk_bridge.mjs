import { stdin, stdout, stderr, exit } from "node:process";
import { chdir } from "node:process";
import { readFile } from "node:fs/promises";
import { extname } from "node:path";

const CLAUDE_SETTING_SOURCES = ["user", "project"];

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

function createLineReader() {
  const queue = [];
  let pending = null;
  let buffer = "";
  let ended = false;

  const deliver = (line) => {
    if (pending) {
      const resolve = pending;
      pending = null;
      resolve(line);
    } else if (line != null) {
      queue.push(line);
    }
  };

  stdin.setEncoding("utf8");
  stdin.on("data", (chunk) => {
    buffer += chunk;
    while (true) {
      const newline = buffer.indexOf("\n");
      if (newline < 0) break;
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (line) deliver(line);
    }
  });
  stdin.on("end", () => {
    ended = true;
    if (buffer.trim()) {
      const line = buffer.trim();
      buffer = "";
      deliver(line);
    }
    deliver(null);
  });
  stdin.on("error", () => {
    ended = true;
    deliver(null);
  });

  return {
    async nextLine() {
      if (queue.length > 0) return queue.shift();
      if (ended) return null;
      return new Promise((resolve) => {
        pending = resolve;
      });
    },
    tryNextLine() {
      return queue.length > 0 ? queue.shift() : null;
    },
  };
}

function extractFollowupPrompt(payload) {
  if (!payload || typeof payload !== "object") return null;
  if (
    payload.type === "end" ||
    payload.type === "await_followups" ||
    payload.type === "awaitFollowups"
  ) {
    return null;
  }
  if (typeof payload.prompt === "string" && payload.prompt.trim()) {
    return payload.prompt.trim();
  }
  return null;
}

function isAwaitFollowupsControl(payload) {
  return (
    payload &&
    typeof payload === "object" &&
    (payload.type === "await_followups" || payload.type === "awaitFollowups")
  );
}

function applyAwaitFollowupsControl(payload, current) {
  if (!isAwaitFollowupsControl(payload)) return current;
  if (typeof payload.enabled === "boolean") return payload.enabled;
  if (typeof payload.value === "boolean") return payload.value;
  return true;
}

function normalizeFileChangeKind(kind) {
  const value = String(kind ?? "")
    .trim()
    .toLowerCase();
  if (["add", "added", "create", "created"].includes(value)) return "added";
  if (
    ["modify", "modified", "update", "updated", "change", "changed", "edit", "edited"].includes(
      value,
    )
  )
    return "modified";
  if (["delete", "deleted", "remove", "removed"].includes(value)) return "deleted";
  if (["rename", "renamed", "move", "moved"].includes(value)) return "renamed";
  return null;
}

function imageMediaType(path) {
  const ext = extname(String(path || "")).toLowerCase();
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".webp") return "image/webp";
  if (ext === ".gif") return "image/gif";
  if (ext === ".png") return "image/png";
  return "application/octet-stream";
}

async function buildPrompt(payload) {
  const imagePaths = Array.isArray(payload.imagePaths) ? payload.imagePaths : [];
  if (imagePaths.length === 0) {
    return payload.prompt;
  }

  const content = [{ type: "text", text: payload.prompt || "" }];
  for (const path of imagePaths) {
    const data = await readFile(path, { encoding: "base64" });
    content.push({
      type: "image",
      source: {
        type: "base64",
        media_type: imageMediaType(path),
        data,
      },
    });
  }

  return (async function* promptStream() {
    yield {
      type: "user",
      message: {
        role: "user",
        content,
      },
      parent_tool_use_id: null,
    };
  })();
}

function extractFileChangesFromToolUse(block) {
  if (!block || block.type !== "tool_use") return null;
  const name = block.name;
  const input = block.input;
  if (!input) return null;

  const path = input.file_path || input.path || input.filePath;
  if (!path) return null;

  if (name === "Write") {
    return { kind: "added", path, previous_path: null };
  }
  if (name === "Edit" || name === "MultiEdit" || name === "NotebookEdit") {
    return { kind: "modified", path, previous_path: null };
  }
  return null;
}

async function runSession(payload) {
  const { query } = await import("@anthropic-ai/claude-agent-sdk");

  const options = {
    model: payload.model || "sonnet",
    settingSources: CLAUDE_SETTING_SOURCES,
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    maxTurns: payload.maxTurns || 50,
  };

  if (payload.workingDirectory) {
    chdir(payload.workingDirectory);
    options.cwd = payload.workingDirectory;
  }

  if (payload.systemPrompt) {
    options.systemPrompt = payload.systemPrompt;
  }

  if (payload.claudePathOverride) {
    options.pathToClaudeCodeExecutable = payload.claudePathOverride;
  }

  if (payload.effort === "auto") {
    options.thinking = { type: "adaptive" };
  } else if (payload.effort) {
    options.effort = payload.effort;
  } else if (payload.thinkingBudgetTokens && payload.thinkingBudgetTokens > 0) {
    options.thinking = {
      type: "enabled",
      budgetTokens: payload.thinkingBudgetTokens,
    };
  }

  if (payload.resumeSessionId) {
    options.resume = payload.resumeSessionId;
  }

  const queryOptions = {
    prompt: await buildPrompt(payload),
    options,
  };

  const fileChanges = new Map();
  const agentMessages = new Map();
  let sessionId = payload.resumeSessionId || null;

  emit("[SDK] 任务已提交，等待 Claude 响应...");

  for await (const message of query(queryOptions)) {
    switch (message.type) {
      case "system":
        if (message.subtype === "init" && message.session_id) {
          sessionId = message.session_id;
          emit(`session id: ${message.session_id}`);
        }
        break;

      case "assistant":
        if (message.message && Array.isArray(message.message.content)) {
          for (const block of message.message.content) {
            if (block.type === "text" && block.text) {
              const previous = agentMessages.get(message.message.id) ?? "";
              const next = block.text;
              const delta = next.startsWith(previous) ? next.slice(previous.length) : next;
              agentMessages.set(message.message.id, next);
              if (delta.trim()) {
                emitMultiline(delta);
              }
            }
            if (block.type === "thinking" && block.thinking) {
              emit(`[思考] ${block.thinking.substring(0, 200)}`);
            }
            if (block.type === "tool_use") {
              const change = extractFileChangesFromToolUse(block);
              if (change) {
                const key = change.path;
                if (!fileChanges.has(key) || fileChanges.get(key) !== change.kind) {
                  fileChanges.set(key, change.kind);
                  emit(
                    `[CLAUDE_FILE_CHANGE] ${JSON.stringify({
                      changes: [change],
                    })}`,
                  );
                  emit(`[文件变更] ${change.kind}:${change.path}`);
                }
              }

              if (block.name === "Bash") {
                emit(`[命令] ${block.input?.command || "(unknown)"}`);
              } else if (block.name === "Read") {
                emit(`[读取] ${block.input?.file_path || "(unknown)"}`);
              } else if (block.name === "Write") {
                emit(`[写入] ${block.input?.file_path || "(unknown)"}`);
              } else if (block.name === "Edit") {
                emit(`[编辑] ${block.input?.file_path || "(unknown)"}`);
              }
            }
          }
        }
        break;

      case "user":
        if (message.message && Array.isArray(message.message.content)) {
          for (const block of message.message.content) {
            if (block.type === "tool_result" && block.content) {
              const textParts = Array.isArray(block.content)
                ? block.content
                    .filter((c) => c.type === "text")
                    .map((c) => c.text)
                    .join("\n")
                : typeof block.content === "string"
                  ? block.content
                  : "";
              if (textParts.trim()) {
                const lines = textParts.split("\n");
                const preview = lines.slice(0, 20);
                for (const line of preview) {
                  emit(line);
                }
                if (lines.length > 20) {
                  emit(`... (共 ${lines.length} 行)`);
                }
              }
            }
          }
        }
        break;

      case "result":
        if (message.subtype === "success") {
          emit("[SDK] 执行完成");
          if (message.result) {
            emitMultiline(message.result);
          }
        } else if (message.subtype === "error_max_turns") {
          emit("[SDK] 已达最大轮次限制");
        } else if (message.subtype === "error_during_execution") {
          emitError(`[ERROR] 执行过程中出错`);
        }
        if (message.session_id) {
          sessionId = message.session_id;
          emit(`session id: ${message.session_id}`);
        }
        break;

      default:
        break;
    }
  }

  return sessionId;
}

async function runOneShot(payload) {
  const { query } = await import("@anthropic-ai/claude-agent-sdk");

  const options = {
    model: payload.model || "sonnet",
    settingSources: CLAUDE_SETTING_SOURCES,
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    maxTurns: payload.maxTurns || 3,
  };

  if (payload.workingDirectory) {
    chdir(payload.workingDirectory);
    options.cwd = payload.workingDirectory;
  }

  if (payload.claudePathOverride) {
    options.pathToClaudeCodeExecutable = payload.claudePathOverride;
  }

  if (payload.effort === "auto") {
    options.thinking = { type: "adaptive" };
  } else if (payload.effort) {
    options.effort = payload.effort;
  } else if (payload.thinkingBudgetTokens && payload.thinkingBudgetTokens > 0) {
    options.thinking = {
      type: "enabled",
      budgetTokens: payload.thinkingBudgetTokens,
    };
  }

  let resultText = "";
  for await (const message of query({ prompt: payload.prompt, options })) {
    if (message.type === "result" && message.subtype === "success") {
      resultText = message.result || "";
    }
  }

  stdout.write(
    JSON.stringify({
      ok: true,
      text: resultText,
    }),
  );
}

async function main() {
  let mode = "one_shot";
  try {
    const lineReader = createLineReader();
    const firstLine = await lineReader.nextLine();
    if (!firstLine) {
      throw new Error("missing input payload");
    }
    const payload = JSON.parse(firstLine);
    mode = payload.mode === "session" ? "session" : "one_shot";

    if (mode === "session") {
      // Task-linked default false (auto-exit). Log UI can enable via await_followups control.
      let shouldAwait = payload.awaitFollowups === true;

      const runFollowupFromLine = async (sessionId, line) => {
        let followup;
        try {
          followup = JSON.parse(line);
        } catch (error) {
          emitError(`[ERROR] 无法解析会话输入: ${error?.message || error}`);
          return sessionId;
        }
        if (isAwaitFollowupsControl(followup)) {
          shouldAwait = applyAwaitFollowupsControl(followup, shouldAwait);
          return sessionId;
        }
        const prompt = extractFollowupPrompt(followup);
        if (!prompt) {
          return sessionId;
        }
        if (!sessionId) {
          emitError("[ERROR] 缺少 session id，无法继续会话中输入");
          return sessionId;
        }
        emit("[SDK] 收到会话中输入，继续执行...");
        return await runSession({
          ...payload,
          prompt,
          imagePaths: [],
          resumeSessionId: sessionId,
        });
      };

      const drainQueuedFollowups = async (sessionId) => {
        let current = sessionId;
        while (true) {
          const queued = lineReader.tryNextLine();
          if (!queued) break;
          current = await runFollowupFromLine(current, queued);
        }
        return current;
      };

      let sessionId = await runSession(payload);
      sessionId = await drainQueuedFollowups(sessionId);

      if (!shouldAwait) {
        // Open stdin listeners keep the event loop alive; must exit so automation
        // observes process end (do not rely on main() return while stdin is open).
        exit(0);
      }

      emit("[SDK] 等待会话中输入...");
      while (true) {
        const line = await lineReader.nextLine();
        if (!line) {
          exit(0);
        }
        sessionId = await runFollowupFromLine(sessionId, line);
        if (!shouldAwait) {
          exit(0);
        }
        sessionId = await drainQueuedFollowups(sessionId);
        if (!shouldAwait) {
          exit(0);
        }
        emit("[SDK] 等待会话中输入...");
      }
    } else {
      await runOneShot(payload);
    }
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
