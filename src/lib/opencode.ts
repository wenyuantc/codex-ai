import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Types ----

export interface OpenCodeSettings {
    sdk_enabled: boolean;
    default_model: string;
    host: string;
    port: number;
    sdk_install_dir: string;
    node_path_override: string | null;
}

export interface UpdateOpenCodeSettings {
    sdk_enabled?: boolean;
    default_model?: string;
    host?: string;
    port?: number;
    node_path_override?: string | null;
    sdk_install_dir?: string | null;
}

export interface OpenCodeHealthCheck {
    sdk_installed: boolean;
    sdk_version: string | null;
    node_available: boolean;
    node_version: string | null;
    sdk_install_dir: string;
    effective_provider: string;
    sdk_status_message: string;
    checked_at: string;
}

export interface OpenCodeSdkInstallResult {
    sdk_installed: boolean;
    sdk_version: string | null;
    install_dir: string;
    node_version: string | null;
    message: string;
}

export interface OpenCodeOutputEvent {
    employee_id: string;
    task_id: string | null;
    session_kind: string;
    session_record_id: string;
    session_event_id: string | null;
    line: string;
}

export interface OpenCodeExitEvent {
    employee_id: string;
    task_id: string | null;
    session_kind: string;
    session_record_id: string;
    session_event_id: string | null;
    line: string | null;
    code: number | null;
}

export interface OpenCodeSessionEvent {
    employee_id: string;
    task_id: string | null;
    session_kind: string;
    session_record_id: string;
    session_id: string;
}

export interface OpenCodeModelCapabilities {
    reasoning: boolean;
}

export interface OpenCodeModelInfo {
    value: string;
    label: string;
    providerId: string;
    providerName: string;
    modelId: string;
    capabilities?: OpenCodeModelCapabilities | null;
}

// ---- API Functions ----

export function getOpenCodeSettings(): Promise<OpenCodeSettings> {
    return invoke<OpenCodeSettings>("get_opencode_settings");
}

export function updateOpenCodeSettings(updates: UpdateOpenCodeSettings): Promise<OpenCodeSettings> {
    return invoke<OpenCodeSettings>("update_opencode_settings", { updates });
}

export function checkOpenCodeSdkHealth(): Promise<OpenCodeHealthCheck> {
    return invoke<OpenCodeHealthCheck>("check_opencode_sdk_health");
}

export function getOpenCodeModels(): Promise<OpenCodeModelInfo[]> {
    return invoke<OpenCodeModelInfo[]>("get_opencode_models");
}

export function installOpenCodeSdk(): Promise<OpenCodeSdkInstallResult> {
    return invoke<OpenCodeSdkInstallResult>("install_opencode_sdk");
}

export function startOpenCode(params: {
    employeeId: string;
    taskDescription: string;
    model?: string;
    workingDir?: string;
    taskId?: string;
    taskGitContextId?: string;
    resumeSessionId?: string;
    imagePaths?: string[];
}): Promise<void> {
    return invoke<void>("start_opencode", params);
}

export function stopOpenCodeSession(sessionRecordId: string): Promise<void> {
    return invoke<void>("stop_opencode_session", { sessionRecordId });
}

export function stopOpenCode(employeeId: string): Promise<void> {
    return invoke<void>("stop_opencode", { employeeId });
}

// ---- Event Listeners ----

export function onOpenCodeOutput(
    callback: (event: OpenCodeOutputEvent) => void
): Promise<UnlistenFn> {
    return listen<OpenCodeOutputEvent>("opencode-stdout", (event) => {
        callback(event.payload);
    });
}

export function onOpenCodeSession(
    callback: (event: OpenCodeSessionEvent) => void
): Promise<UnlistenFn> {
    return listen<OpenCodeSessionEvent>("opencode-session", (event) => {
        callback(event.payload);
    });
}

export function onOpenCodeExit(
    callback: (event: OpenCodeExitEvent) => void
): Promise<UnlistenFn> {
    return listen<OpenCodeExitEvent>("opencode-exit", (event) => {
        callback(event.payload);
    });
}
