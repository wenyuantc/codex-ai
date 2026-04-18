import { useEffect, useRef, useState } from "react";

import {
  getCodexSessionExecutionChangeHistory,
  getCodexSessionFileChangeDetail,
} from "@/lib/backend";
import type {
  CodexSessionFileChange,
  CodexSessionFileChangeDetail,
  CodexSessionListItem,
  TaskExecutionChangeHistoryItem,
} from "@/lib/types";
import { formatDate, isArtifactCaptureLimited } from "@/lib/utils";
import { TaskExecutionChangeDetailDialog } from "@/components/tasks/detail/TaskExecutionChangeDetailDialog";
import { TaskFileChangeHistoryPanel } from "@/components/tasks/detail/TaskFileChangeHistoryPanel";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface SessionExecutionChangesDialogProps {
  open: boolean;
  session: CodexSessionListItem | null;
  onOpenChange: (open: boolean) => void;
}

export function SessionExecutionChangesDialog({
  open,
  session,
  onOpenChange,
}: SessionExecutionChangesDialogProps) {
  const [history, setHistory] = useState<TaskExecutionChangeHistoryItem | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [selectedChange, setSelectedChange] = useState<CodexSessionFileChange | null>(null);
  const [changeDetail, setChangeDetail] = useState<CodexSessionFileChangeDetail | null>(null);
  const [changeDetailOpen, setChangeDetailOpen] = useState(false);
  const [changeDetailLoading, setChangeDetailLoading] = useState(false);
  const [changeDetailError, setChangeDetailError] = useState<string | null>(null);
  const historyRequestIdRef = useRef(0);
  const changeDetailRequestIdRef = useRef(0);

  const loadHistory = async (target: CodexSessionListItem) => {
    const requestId = historyRequestIdRef.current + 1;
    historyRequestIdRef.current = requestId;
    setHistoryLoading(true);
    setHistoryError(null);

    try {
      const result = await getCodexSessionExecutionChangeHistory(target.session_record_id);
      if (historyRequestIdRef.current !== requestId) {
        return;
      }
      setHistory(result);
    } catch (error) {
      if (historyRequestIdRef.current !== requestId) {
        return;
      }
      setHistory(null);
      setHistoryError(error instanceof Error ? error.message : "读取对话文件改动失败");
    } finally {
      if (historyRequestIdRef.current === requestId) {
        setHistoryLoading(false);
      }
    }
  };

  const handleOpenChangeDetail = async (change: CodexSessionFileChange) => {
    const requestId = changeDetailRequestIdRef.current + 1;
    changeDetailRequestIdRef.current = requestId;
    setSelectedChange(change);
    setChangeDetail(null);
    setChangeDetailOpen(true);
    setChangeDetailLoading(true);
    setChangeDetailError(null);

    try {
      const detail = await getCodexSessionFileChangeDetail(change.id);
      if (changeDetailRequestIdRef.current !== requestId) {
        return;
      }
      setChangeDetail(detail);
    } catch (error) {
      if (changeDetailRequestIdRef.current !== requestId) {
        return;
      }
      setChangeDetail(null);
      setChangeDetailError(error instanceof Error ? error.message : "读取文件 diff 详情失败");
    } finally {
      if (changeDetailRequestIdRef.current === requestId) {
        setChangeDetailLoading(false);
      }
    }
  };

  const handleChangeDetailOpenChange = (nextOpen: boolean) => {
    setChangeDetailOpen(nextOpen);
    if (!nextOpen) {
      changeDetailRequestIdRef.current += 1;
      setChangeDetailLoading(false);
      setChangeDetailError(null);
      setChangeDetail(null);
      setSelectedChange(null);
    }
  };

  useEffect(() => {
    if (!open || !session) {
      return;
    }

    void loadHistory(session);
  }, [open, session]);

  useEffect(() => {
    if (!open) {
      historyRequestIdRef.current += 1;
      changeDetailRequestIdRef.current += 1;
      setHistory(null);
      setHistoryLoading(false);
      setHistoryError(null);
      setSelectedChange(null);
      setChangeDetail(null);
      setChangeDetailOpen(false);
      setChangeDetailLoading(false);
      setChangeDetailError(null);
    }
  }, [open]);

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]"
          showCloseButton
        >
          <DialogHeader>
            <DialogTitle>对话改动文件</DialogTitle>
            <DialogDescription>
              {session
                ? `查看执行对话“${session.display_name}”保存下来的改动文件记录，并继续点进文件级 diff。`
                : "查看执行对话的改动文件记录"}
            </DialogDescription>
          </DialogHeader>

          {session ? (
            <div className="grid gap-2 text-xs md:grid-cols-2">
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2">
                <div className="font-medium text-foreground">对话</div>
                <div className="mt-1 font-mono text-muted-foreground">{session.session_id}</div>
                <div className="mt-1 font-mono text-muted-foreground">
                  记录 ID: {session.session_record_id}
                </div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2">
                <div className="font-medium text-foreground">最近更新时间</div>
                <div className="mt-1 text-muted-foreground">{formatDate(session.last_updated_at)}</div>
                <div className="mt-1 text-muted-foreground">
                  员工：{session.employee_name ?? "未绑定"} · 任务：{session.task_title ?? "无关联任务"}
                </div>
                {session.target_host_label && (
                  <div className="mt-1 text-muted-foreground">主机：{session.target_host_label}</div>
                )}
              </div>
              {session.working_dir && (
                <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-muted-foreground md:col-span-2">
                  <div className="font-medium text-foreground">工作目录</div>
                  <div className="mt-1 break-all font-mono">{session.working_dir}</div>
                </div>
              )}
              {isArtifactCaptureLimited(session.artifact_capture_mode) && (
                <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-amber-800 md:col-span-2">
                  远程变更明细受限：SSH v1 只保证远程执行与对话元数据，不承诺本地级 diff 和文件快照。
                </div>
              )}
            </div>
          ) : (
            <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-sm text-muted-foreground">
              当前没有可查看的对话。
            </div>
          )}

          <TaskFileChangeHistoryPanel
            title="执行对话改动文件"
            description={session?.artifact_capture_mode === "local_full"
              ? "SDK 对话按 Codex 事件精确记录；CLI 对话仅在缺少结构化事件时回退为 Git 快照估算。"
              : session?.artifact_capture_mode === "ssh_full"
                ? "当前远程对话已保存文件级明细；SDK 对话优先使用 Codex 事件，CLI 对话按远程 Git 快照估算。"
                : "当前是远程对话，变更采集能力受限，仅展示 SSH v1 可提供的记录。"}
            history={history ? [history] : []}
            loading={historyLoading}
            error={historyError}
            emptyText="该执行对话还没有文件改动记录。"
            loadingText="正在加载该对话的文件改动..."
            onRefresh={() => {
              if (session) {
                void loadHistory(session);
              }
            }}
            onOpenChangeDetail={handleOpenChangeDetail}
          />
        </DialogContent>
      </Dialog>

      <TaskExecutionChangeDetailDialog
        open={changeDetailOpen}
        loading={changeDetailLoading}
        error={changeDetailError}
        detail={
          changeDetail ?? (selectedChange
            ? {
                change: selectedChange,
                working_dir: session?.working_dir ?? null,
                absolute_path: null,
                previous_absolute_path: null,
                before_status: "missing",
                before_text: null,
                before_truncated: false,
                after_status: "missing",
                after_text: null,
                after_truncated: false,
                diff_text: null,
                diff_truncated: false,
                snapshot_status: "unavailable",
                snapshot_message: null,
              }
            : null)
        }
        onOpenChange={handleChangeDetailOpenChange}
      />
    </>
  );
}
