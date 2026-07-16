import type { CodexSessionFileChange, TaskExecutionChangeHistoryItem } from "@/lib/types";
import { formatDate, isArtifactCaptureLimited } from "@/lib/utils";
import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";
import {
  getExecutionChangeCaptureModeDescription,
  getExecutionChangeCaptureModeLabel,
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getSessionStatusLabel,
} from "./taskDetailViewHelpers";

interface TaskFileChangeHistoryPanelProps {
  title: string;
  description: string;
  history: TaskExecutionChangeHistoryItem[];
  loading: boolean;
  error: string | null;
  emptyText: string;
  loadingText?: string;
  onRefresh: () => void;
  onOpenChangeDetail: (change: CodexSessionFileChange) => void;
  /** Optional override; otherwise inferred from history sessions. */
  showSshArtifactNotice?: boolean;
}

export function TaskFileChangeHistoryPanel({
  title,
  description,
  history,
  loading,
  error,
  emptyText,
  loadingText = "正在加载修改文件历史...",
  onRefresh,
  onOpenChangeDetail,
  showSshArtifactNotice,
}: TaskFileChangeHistoryPanelProps) {
  const limitedFromHistory = history.some((item) =>
    isArtifactCaptureLimited(item.session.artifact_capture_mode),
  );
  const shouldShowSshNotice = showSshArtifactNotice ?? limitedFromHistory;
  const limitedMode = history.find((item) =>
    isArtifactCaptureLimited(item.session.artifact_capture_mode),
  )?.session.artifact_capture_mode;

  return (
    <div className="space-y-3 rounded-md border border-border/70 bg-muted/20 p-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="space-y-1">
          <p className="text-sm font-medium">{title}</p>
          <p className="text-[11px] text-muted-foreground">{description}</p>
        </div>
        <button
          type="button"
          onClick={onRefresh}
          className="text-[11px] text-primary hover:underline disabled:opacity-50"
          disabled={loading}
        >
          {loading ? "刷新中..." : "刷新"}
        </button>
      </div>

      {shouldShowSshNotice && (
        <SshArtifactLimitedNotice
          artifactCaptureMode={limitedMode}
          force={showSshArtifactNotice === true && !limitedMode}
        />
      )}

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      {history.length > 0 ? (
        <div className="space-y-3">
          {history.map((item) => (
            <div key={item.session.id} className="rounded-md border border-border bg-background/70 p-3">
              <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted-foreground">
                <span>
                  {formatDate(item.session.started_at)}
                  {" · "}
                  {getSessionStatusLabel(item.session.status)}
                </span>
                <span className="font-mono">{item.session.cli_session_id ?? item.session.id}</span>
              </div>
              <div className="mt-2 rounded-md border border-border/60 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground">
                <span className="font-medium text-foreground">
                  {getExecutionChangeCaptureModeLabel(item.capture_mode)}
                </span>
                {" · "}
                {getExecutionChangeCaptureModeDescription(item.capture_mode)}
              </div>

              {item.changes.length > 0 ? (
                <div className="mt-3 space-y-2">
                  {item.changes.map((change) => (
                    <button
                      type="button"
                      key={change.id}
                      onClick={() => onOpenChangeDetail(change)}
                      className="flex w-full flex-col gap-1 rounded-md border border-border/60 bg-background px-3 py-2 text-left text-xs transition-colors hover:border-primary/40 hover:bg-muted/30"
                    >
                      <div className="flex flex-wrap items-center gap-2">
                        <span
                          className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getExecutionChangeTypeClassName(change.change_type)}`}
                        >
                          {getExecutionChangeTypeLabel(change.change_type)}
                        </span>
                        <span className="break-all font-mono text-foreground">{change.path}</span>
                      </div>
                      {change.previous_path && (
                        <div className="text-[11px] text-muted-foreground">
                          原路径：<span className="break-all font-mono">{change.previous_path}</span>
                        </div>
                      )}
                      <div className="text-[11px] text-primary">点击查看该次会话保存的 diff / 内容快照</div>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="mt-3 rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
                  {isArtifactCaptureLimited(item.session.artifact_capture_mode)
                    ? "远程会话未回传文件级变更明细；可查看上方说明，或到远程主机检查 git 状态。"
                    : item.capture_mode === "sdk_event"
                      ? "本次运行没有结构化文件变更记录。"
                      : "本次 Git 快照估算未发现新增文件变更。"}
                </div>
              )}
            </div>
          ))}
        </div>
      ) : loading ? (
        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
          {loadingText}
        </div>
      ) : shouldShowSshNotice ? (
        <SshArtifactLimitedNotice
          artifactCaptureMode={limitedMode}
          force={!limitedMode}
        />
      ) : (
        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
          {emptyText}
        </div>
      )}
    </div>
  );
}
