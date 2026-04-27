import { createOpencode, createOpencodeClient } from "@opencode-ai/sdk";

function emit(type, data) {
    const payload = JSON.stringify({ type, data, timestamp: Date.now() });
    process.stdout.write(payload + "\n");
}

function emitError(message) {
    emit("error", { message });
}

function emitMultiline(prefix, lines) {
    for (const line of lines) {
        emit("stdout", { line: `${prefix} ${line}` });
    }
}

function readInput() {
    return new Promise((resolve, reject) => {
        let input = "";
        process.stdin.setEncoding("utf-8");
        process.stdin.on("data", (chunk) => {
            input += chunk;
        });
        process.stdin.on("end", () => {
            try {
                resolve(JSON.parse(input));
            } catch (error) {
                reject(new Error(`解析输入失败: ${error.message}`));
            }
        });
        process.stdin.on("error", (error) => {
            reject(new Error(`读取输入失败: ${error.message}`));
        });
    });
}

function summarizeFileChange(change) {
    const kind = change.kind || "unknown";
    const path = change.path || "(unknown)";
    return `[OPENCODE_FILE_CHANGE]${JSON.stringify({ changes: [{ path, kind, previous_path: change.previousPath || null }] })}`;
}

function normalizeFileChangeKind(kind) {
    if (!kind) return null;
    const lower = kind.toLowerCase();
    if (["add", "added", "create", "created"].includes(lower)) return "added";
    if (["modify", "modified", "update", "updated", "change", "changed", "edit", "edited"].includes(lower)) return "modified";
    if (["delete", "deleted", "remove", "removed"].includes(lower)) return "deleted";
    if (["rename", "renamed", "move", "moved"].includes(lower)) return "renamed";
    return null;
}

function emitStructuredFileChange(changes) {
    for (const change of changes) {
        const path = change.path || "";
        if (!path) continue;
        const kind = normalizeFileChangeKind(change.kind);
        if (!kind) continue;
        process.stdout.write(summarizeFileChange(change) + "\n");
    }
}

function sessionQuery(config) {
    const directory = config.workingDirectory?.trim();
    return directory ? { directory } : undefined;
}

function modelRef(model) {
    if (!model) return undefined;
    const [providerID, modelID] = model.includes("/")
        ? model.split(/\/(.+)/).filter(Boolean)
        : ["openai", model];
    if (!providerID || !modelID) return undefined;
    return { providerID, modelID };
}

function formatSdkError(error) {
    if (!error) return "OpenCode SDK 返回未知错误";
    if (typeof error === "string") return error;
    if (error.message) return error.message;
    if (error.name && error.data?.message) return `${error.name}: ${error.data.message}`;
    if (error.data?.message) return error.data.message;
    try {
        return JSON.stringify(error);
    } catch {
        return String(error);
    }
}

function collectTextFromPromptResult(result) {
    const message = result?.data || result;
    const parts = message?.parts || message?.info?.parts || [];
    const lines = [];

    for (const part of parts) {
        if (part?.type === "text" && part.text) {
            lines.push(part.text);
        }
    }

    const summary = message?.info?.summary?.body || message?.summary?.body;
    if (summary) {
        lines.push(summary);
    }

    return lines.join("\n").trim();
}

function messageInfo(item) {
    return item?.info || item;
}

function messageId(item) {
    return messageInfo(item)?.id || item?.id;
}

function messageCreated(item) {
    const created = Number(messageInfo(item)?.time?.created || item?.time?.created || 0);
    return Number.isFinite(created) ? created : 0;
}

async function fetchMessageCursor(client, sessionId, config) {
    try {
        const msgs = await client.session.messages({
            path: { id: sessionId },
            query: sessionQuery(config),
        });
        const list = msgs?.data || msgs || [];
        const messages = Array.isArray(list) ? list : [];

        return {
            available: true,
            ids: new Set(messages.map(messageId).filter(Boolean)),
            latestCreated: messages.reduce(
                (latest, item) => Math.max(latest, messageCreated(item)),
                0
            ),
        };
    } catch {
        return { available: false, ids: new Set(), latestCreated: 0 };
    }
}

async function fetchAssistantMessageText(client, sessionId, messageID, config) {
    if (!messageID) return "";

    try {
        const message = await client.session.message({
            path: { id: sessionId, messageID },
            query: sessionQuery(config),
        });
        return collectTextFromPromptResult(message);
    } catch {
        return "";
    }
}

async function fetchLastAssistantText(client, sessionId, config, beforeCursor) {
    if (config.resumeSessionId && beforeCursor?.available === false) {
        return "";
    }

    try {
        const msgs = await client.session.messages({
            path: { id: sessionId },
            query: sessionQuery(config),
        });
        const list = msgs?.data || msgs || [];
        const messages = Array.isArray(list) ? list : [];

        for (const item of messages.slice().reverse()) {
            const info = messageInfo(item);
            if (info?.role && info.role !== "assistant") continue;
            if (beforeCursor?.ids?.has(info?.id)) continue;
            const created = messageCreated(item);
            if (beforeCursor?.latestCreated && created && created < beforeCursor.latestCreated) {
                continue;
            }
            const parts = item?.parts || info?.parts || [];
            const text = parts
                .filter((part) => part?.type === "text" && part.text)
                .map((part) => part.text)
                .join("\n")
                .trim();
            if (text) return text;
            if (info?.summary?.body) return info.summary.body.trim();
        }
    } catch {
        // Best-effort fallback only.
    }

    return "";
}

async function runSession(client, config) {
    const session = config.resumeSessionId
        ? { id: config.resumeSessionId }
        : await client.session.create({
              body: {
                  title: config.prompt?.slice(0, 100) || "OpenCode Session",
              },
              query: sessionQuery(config),
          });

    const sessionId = session?.data?.id || session?.id || config.resumeSessionId;
    emit("session", { session_id: sessionId });

    const systemPrompt = config.systemPrompt;
    const userPrompt = config.prompt || "";
    const promptText = userPrompt;

    let heartbeat = null;
    try {
        const beforeCursor = await fetchMessageCursor(client, sessionId, config);
        heartbeat = setInterval(() => {
            emit("info", { message: "OpenCode 仍在执行，等待响应..." });
        }, 30000);

        const result = await client.session.prompt({
            path: { id: sessionId },
            query: sessionQuery(config),
            body: {
                model: modelRef(config.model),
                system: systemPrompt || undefined,
                parts: [{ type: "text", text: promptText }],
            },
        });

        if (result?.error) {
            emit("error", { message: formatSdkError(result.error).slice(0, 500) });
            emit("done", { session_id: sessionId });
            return;
        }

        const message = result?.data || result;
        if (message?.info?.error) {
            emit("error", { message: formatSdkError(message.info.error).slice(0, 500) });
            emit("done", { session_id: sessionId });
            return;
        }

        let textOutput = collectTextFromPromptResult(result);
        if (!textOutput) {
            textOutput = await fetchAssistantMessageText(
                client,
                sessionId,
                message?.info?.id || message?.id,
                config
            );
        }
        if (!textOutput) {
            textOutput = await fetchLastAssistantText(client, sessionId, config, beforeCursor);
        }

        if (textOutput) {
            for (const line of textOutput.split("\n").filter((l) => l.trim())) {
                emit("stdout", { line: `[OUTPUT] ${line}` });
            }
        } else {
            emit("stdout", { line: "[OUTPUT] (AI 未返回文本内容)" });
        }

        emit("done", { session_id: sessionId });
    } catch (error) {
        emit("error", { message: formatSdkError(error).slice(0, 500) });
        emit("done", { session_id: sessionId });
    } finally {
        if (heartbeat) clearInterval(heartbeat);
    }
}

async function main() {
    try {
        const config = await readInput();

        const { mode, model, host, port, workingDirectory, prompt, systemPrompt, resumeSessionId, imagePaths } = config;

        emit("info", { message: `启动 OpenCode SDK (model: ${model || "default"})` });

        const baseUrl = `http://${host || "127.0.0.1"}:${port || 4096}`;
        let client, server;

        // Try to connect to an existing OpenCode server first
        try {
            client = createOpencodeClient({ baseUrl, directory: workingDirectory || undefined });
            // Quick connectivity check via session.list (doesn't need server start)
            await client.session.list({ query: sessionQuery(config) });
            emit("info", { message: `已连接到运行中的 OpenCode server (${baseUrl})` });
        } catch (connectError) {
            emit("info", { message: `未检测到运行中的 OpenCode server，正在启动新实例...` });
            const modelConfig = model
                ? {
                      model: {
                          modelID: model.includes("/") ? model.split("/").pop() : model,
                          providerID: model.includes("/") ? model.split("/")[0] : "openai",
                      },
                  }
                : {};

            const started = await createOpencode({
                hostname: host || "127.0.0.1",
                port: port || 4096,
                config: modelConfig,
                timeout: 10000,
            });
            client = createOpencodeClient({
                baseUrl: started.server.url,
                directory: workingDirectory || undefined,
            });
            server = started.server;
            emit("info", { message: "OpenCode SDK 新实例已启动" });
        }

        if (mode === "list-providers") {
            const raw = await client.config.providers();
            const payload = raw?.data || raw;
            const providers = payload.providers || [];
            const defaults = payload.default || {};
            const modelList = [];
            for (const provider of providers) {
                const modelIds = Object.keys(provider.models || {});
                for (const modelId of modelIds) {
                    const modelInfo = provider.models[modelId];
                    const label = modelInfo?.name || modelInfo?.id || `${provider.name} ${modelId}`;
                    modelList.push({
                        value: `${provider.id}/${modelId}`,
                        label: label,
                        providerId: provider.id,
                        providerName: provider.name,
                        modelId: modelId,
                        capabilities: modelInfo?.capabilities || {},
                    });
                }
            }
            emit("providers", { providers: modelList, defaults });
            emit("done", { session_id: null });
        } else if (mode === "session" || mode === "resume_session") {
            await runSession(client, config);
        } else {
            const session = await client.session.create({
                body: { title: "one-shot" },
                query: sessionQuery(config),
            });

            const sessionId = session?.data?.id || session?.id;
            const result = await client.session.prompt({
                path: { id: sessionId },
                query: sessionQuery(config),
                body: {
                    model: modelRef(model),
                    parts: [{ type: "text", text: prompt || "" }],
                },
            });

            const message = result?.data || result;
            if (message?.info) {
                const parts = message.info.parts || [];
                for (const part of parts) {
                    if (part.type === "text" && part.text) {
                        for (const line of part.text.split("\n").filter((l) => l.trim())) {
                            emit("stdout", { line: `[OUTPUT] ${line}` });
                        }
                    }
                }
                const summary = message.info.summary;
                if (summary?.body) {
                    for (const line of summary.body.split("\n").filter((l) => l.trim())) {
                        emit("stdout", { line: `[OUTPUT] ${line}` });
                    }
                }
            } else if (message?.error) {
                emit("error", { message: JSON.stringify(message.error) });
            }

            emit("done", { session_id: sessionId });
        }

        if (server) {
            server.close();
        }
    } catch (error) {
        emit("error", { message: error.message });
        emit("done", { session_id: null });
    }

    // Force exit so the parent Rust process sees stdout EOF
    process.exit(0);
}

main();
