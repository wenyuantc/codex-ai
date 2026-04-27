import { createOpencode, createOpencodeClient } from "@opencode-ai/sdk";
import fs from "node:fs";
import net from "node:net";

function writeStdoutLine(line) {
    fs.writeSync(1, `${line}\n`);
}

function emit(type, data) {
    const payload = JSON.stringify({ type, data, timestamp: Date.now() });
    writeStdoutLine(payload);
}

function emitError(message) {
    emit("error", { message });
}

function emitMultiline(prefix, lines) {
    for (const line of lines) {
        emit("stdout", { line: `${prefix} ${line}` });
    }
}

function findFreePort(host) {
    return new Promise((resolve, reject) => {
        const server = net.createServer();
        server.once("error", reject);
        server.listen(0, host, () => {
            const address = server.address();
            const port = typeof address === "object" && address ? address.port : 0;
            server.close((error) => {
                if (error) {
                    reject(error);
                    return;
                }
                if (port > 0) {
                    resolve(port);
                } else {
                    reject(new Error("未能分配 OpenCode 临时端口"));
                }
            });
        });
    });
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
        writeStdoutLine(summarizeFileChange(change));
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

function providerEntries(providers) {
    if (Array.isArray(providers)) {
        return providers.map((provider) => [
            provider?.id || provider?.providerId || provider?.providerID || "",
            provider,
        ]);
    }
    if (providers && typeof providers === "object") {
        return Object.entries(providers);
    }
    return [];
}

function modelEntries(models) {
    if (Array.isArray(models)) {
        return models.map((model) => [
            model?.id || model?.modelId || model?.modelID || model?.value || "",
            model,
        ]);
    }
    if (models && typeof models === "object") {
        return Object.entries(models);
    }
    return [];
}

function splitModelValue(value) {
    const text = String(value || "").trim();
    const slash = text.indexOf("/");
    if (slash > 0 && slash < text.length - 1) {
        return {
            providerId: text.slice(0, slash),
            modelId: text.slice(slash + 1),
        };
    }
    return { providerId: "opencode", modelId: text };
}

function normalizeModelCapabilities(modelInfo) {
    const reasoning = modelInfo?.capabilities?.reasoning;
    return typeof reasoning === "boolean" ? { reasoning } : {};
}

function normalizeProviderModel(provider, providerKey, modelId, modelInfo) {
    const providerValue =
        provider?.id || provider?.providerId || provider?.providerID || providerKey;
    const rawModelId =
        modelId || modelInfo?.id || modelInfo?.modelId || modelInfo?.modelID || "";
    const value =
        modelInfo?.value || (providerValue ? `${providerValue}/${rawModelId}` : "");
    const fallback = splitModelValue(value);
    const providerId =
        provider?.id ||
        provider?.providerId ||
        provider?.providerID ||
        providerKey ||
        modelInfo?.providerId ||
        modelInfo?.providerID ||
        fallback.providerId;
    const normalizedModelId = rawModelId || fallback.modelId;

    if (!providerId || !normalizedModelId) return null;

    const providerName =
        provider?.name || modelInfo?.providerName || modelInfo?.providerID || providerId;
    const label =
        modelInfo?.label || modelInfo?.name || modelInfo?.id || `${providerName} ${normalizedModelId}`;

    return {
        value: value || `${providerId}/${normalizedModelId}`,
        label,
        providerId,
        providerName,
        modelId: normalizedModelId,
        capabilities: normalizeModelCapabilities(modelInfo),
    };
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

async function collectPromptText(client, sessionId, config, result, beforeCursor) {
    const message = result?.data || result;

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

    return textOutput;
}

function emitTextOutput(textOutput) {
    for (const line of textOutput.split("\n").filter((l) => l.trim())) {
        emit("stdout", { line: `[OUTPUT] ${line}` });
    }
}

function opencodeRuntimeConfig(model, effort) {
    const normalizedModel = String(model || "").trim();
    const ref = modelRef(normalizedModel);
    const config = normalizedModel ? { model: normalizedModel } : {};
    const normalizedEffort = String(effort || "").trim();
    if (!ref || !normalizedEffort || normalizedEffort === "default") {
        return config;
    }

    config.provider = {
        [ref.providerID]: {
            models: {
                [ref.modelID]: {
                    options: {
                        reasoning_effort: normalizedEffort,
                    },
                },
            },
        },
    };
    return config;
}

function isOpencodeConfigContentError(error) {
    const data = error?.data || error?.cause?.data;
    return (
        (error?.name === "ConfigInvalidError" || error?.cause?.name === "ConfigInvalidError") &&
        data?.path === "OPENCODE_CONFIG_CONTENT"
    );
}

function configContentError(error) {
    const wrapped = new Error(formatSdkError(error));
    wrapped.name = "OpenCodeConfigContentError";
    wrapped.cause = error;
    return wrapped;
}

async function promptSession(client, sessionId, config, includeSystemPrompt) {
    return client.session.prompt({
        path: { id: sessionId },
        query: sessionQuery(config),
        body: {
            model: modelRef(config.model),
            system: includeSystemPrompt ? config.systemPrompt || undefined : undefined,
            parts: [{ type: "text", text: config.prompt || "" }],
        },
    });
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

    let heartbeat = null;
    try {
        const beforeCursor = await fetchMessageCursor(client, sessionId, config);
        heartbeat = setInterval(() => {
            emit("info", { message: "OpenCode 仍在执行，等待响应..." });
        }, 30000);

        const result = await promptSession(client, sessionId, config, true);

        if (result?.error) {
            if (isOpencodeConfigContentError(result.error)) {
                throw configContentError(result.error);
            }
            emit("error", { message: formatSdkError(result.error).slice(0, 500) });
            emit("done", { session_id: sessionId });
            return;
        }

        const message = result?.data || result;
        if (message?.info?.error) {
            if (isOpencodeConfigContentError(message.info.error)) {
                throw configContentError(message.info.error);
            }
            emit("error", { message: formatSdkError(message.info.error).slice(0, 500) });
            emit("done", { session_id: sessionId });
            return;
        }

        const textOutput = await collectPromptText(client, sessionId, config, result, beforeCursor);

        if (textOutput) {
            emitTextOutput(textOutput);
        } else {
            emit("stdout", { line: "[OUTPUT] (AI 未返回文本内容)" });
        }

        emit("done", { session_id: sessionId });
    } catch (error) {
        if (isOpencodeConfigContentError(error)) {
            throw error;
        }
        emit("error", { message: formatSdkError(error).slice(0, 500) });
        emit("done", { session_id: sessionId });
    } finally {
        if (heartbeat) clearInterval(heartbeat);
    }
}

async function runOneShot(client, config) {
    const session = await client.session.create({
        body: { title: "one-shot" },
        query: sessionQuery(config),
    });

    const sessionId = session?.data?.id || session?.id;
    emit("session", { session_id: sessionId });

    const beforeCursor = await fetchMessageCursor(client, sessionId, config);
    const result = await promptSession(client, sessionId, config, Boolean(config.systemPrompt));

    if (result?.error) {
        if (isOpencodeConfigContentError(result.error)) {
            throw configContentError(result.error);
        }
        emit("error", { message: formatSdkError(result.error).slice(0, 500) });
        emit("done", { session_id: sessionId });
        return;
    }

    const message = result?.data || result;
    if (message?.info?.error) {
        if (isOpencodeConfigContentError(message.info.error)) {
            throw configContentError(message.info.error);
        }
        emit("error", { message: formatSdkError(message.info.error).slice(0, 500) });
        emit("done", { session_id: sessionId });
        return;
    }

    const textOutput = await collectPromptText(client, sessionId, config, result, beforeCursor);
    if (textOutput) {
        emitTextOutput(textOutput);
    }

    emit("done", { session_id: sessionId });
}

async function main() {
    try {
        const config = await readInput();

        const { mode, model, host, port, workingDirectory, reasoningEffort } = config;

        emit("info", { message: `启动 OpenCode SDK (model: ${model || "default"})` });

        const baseUrl = `http://${host || "127.0.0.1"}:${port || 4096}`;
        let client, server;
        let connectedExistingServer = false;

        const startManagedServer = async (portToUse, reason) => {
            if (reason) {
                emit("info", { message: reason });
            }
            const started = await createOpencode({
                hostname: host || "127.0.0.1",
                port: portToUse,
                config: opencodeRuntimeConfig(model, reasoningEffort),
                timeout: 10000,
            });
            client = createOpencodeClient({
                baseUrl: started.server.url,
                directory: workingDirectory || undefined,
            });
            server = started.server;
            connectedExistingServer = false;
            emit("info", { message: `OpenCode SDK 新实例已启动 (${started.server.url})` });
        };

        const runRequestedMode = async () => {
            if (mode === "list-providers") {
                const raw = await client.config.providers();
                const payload = raw?.data || raw;
                const providers = payload.providers || payload;
                const defaults = payload.default || payload.defaults || {};
                const modelList = [];
                for (const [providerKey, provider] of providerEntries(providers)) {
                    if (provider?.value && !provider?.models) {
                        const model = normalizeProviderModel(provider, providerKey, null, provider);
                        if (model) modelList.push(model);
                        continue;
                    }

                    for (const [modelId, modelInfo] of modelEntries(
                        provider?.models || provider?.model
                    )) {
                        const model = normalizeProviderModel(provider, providerKey, modelId, modelInfo);
                        if (model) modelList.push(model);
                    }
                }
                emit("providers", { providers: modelList, defaults });
                emit("done", { session_id: null });
            } else if (mode === "session" || mode === "resume_session") {
                await runSession(client, config);
            } else {
                await runOneShot(client, config);
            }
        };

        // Try to connect to an existing OpenCode server first
        try {
            client = createOpencodeClient({ baseUrl, directory: workingDirectory || undefined });
            // Quick connectivity check via session.list (doesn't need server start)
            await client.session.list({ query: sessionQuery(config) });
            connectedExistingServer = true;
            emit("info", { message: `已连接到运行中的 OpenCode server (${baseUrl})` });
        } catch (connectError) {
            emit("info", { message: `未检测到运行中的 OpenCode server，正在启动新实例...` });
            await startManagedServer(port || 4096);
        }

        try {
            await runRequestedMode();
        } catch (error) {
            if (connectedExistingServer && isOpencodeConfigContentError(error)) {
                emit("info", {
                    message:
                        "运行中的 OpenCode server 使用旧版无效配置，已切换到隔离临时实例重试...",
                });
                const fallbackPort = await findFreePort(host || "127.0.0.1");
                await startManagedServer(fallbackPort);
                await runRequestedMode();
            } else {
                throw error;
            }
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
