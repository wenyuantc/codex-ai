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

async function runSession(client, config) {
    const session = await client.session.create({
        body: {
            title: config.prompt?.slice(0, 100) || "OpenCode Session",
            model: config.model ? { modelID: config.model.split("/").pop(), providerID: config.model.split("/")[0] } : undefined,
        },
    });

    const sessionId = session?.data?.id || session?.id;
    emit("session", { session_id: sessionId });

    const systemPrompt = config.systemPrompt;
    const userPrompt = config.prompt || "";
    const fullPrompt = systemPrompt ? `${systemPrompt}\n\n${userPrompt}` : userPrompt;

    // Send prompt with 60s timeout
    const promptPromise = client.session.prompt({
        path: { id: sessionId },
        body: {
            parts: [{ type: "text", text: fullPrompt }],
        },
    });

    const timeout = new Promise((_, reject) =>
        setTimeout(() => reject(new Error("Prompt timeout (60s)")), 60000)
    );

    const result = await Promise.race([promptPromise, timeout]);

    if (result?.error) {
        emit("error", { message: JSON.stringify(result.error).slice(0, 500) });
        emit("done", { session_id: sessionId });
        return;
    }

    // Poll messages for AI response
    let textOutput = "";
    for (let i = 0; i < 30; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        try {
            const msgs = await client.session.messages({ path: { id: sessionId } });
            const list = msgs?.data || msgs || [];
            const lastMsg = Array.isArray(list) ? list[list.length - 1] : null;

            if (lastMsg && Array.isArray(lastMsg.parts)) {
                const textParts = lastMsg.parts.filter((p) => p.type === "text" && p.text);
                if (textParts.length > 0) {
                    textOutput = textParts.map((p) => p.text).join("\n");
                    break;
                }
            }
        } catch {
            // retry
        }
    }

    if (textOutput) {
        for (const line of textOutput.split("\n").filter((l) => l.trim())) {
            emit("stdout", { line: `[OUTPUT] ${line}` });
        }
    } else {
        emit("stdout", { line: "[OUTPUT] (AI 未返回文本内容)" });
    }

    emit("done", { session_id: sessionId });
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
            client = createOpencodeClient({ baseUrl });
            // Quick connectivity check via session.list (doesn't need server start)
            await client.session.list();
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
            client = started.client;
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
            });

            const sessionId = session?.data?.id || session?.id;
            const result = await client.session.prompt({
                path: { sessionID: sessionId },
                body: {
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
