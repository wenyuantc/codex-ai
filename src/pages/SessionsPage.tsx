import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Loader2, RefreshCw } from "lucide-react";

import { SessionContinueDialog } from "@/components/sessions/SessionContinueDialog";
import { SessionLogDialog, type SessionLogTarget } from "@/components/sessions/SessionLogDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { listCodexSessions, prepareCodexSessionResume } from "@/lib/backend";
import { startCodex } from "@/lib/codex";
import type { CodexSessionListItem, CodexSessionResumeStatus } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";

const PAGE_SIZE = 10;

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

function formatSessionKind(kind: CodexSessionListItem["session_kind"]) {
  return kind === "review" ? "审核" : "执行";
}

function formatSessionStatus(status: string) {
  switch (status) {
    case "pending":
      return "待启动";
    case "running":
      return "运行中";
    case "stopping":
      return "停止中";
    case "exited":
      return "已结束";
    case "failed":
      return "失败";
    default:
      return status;
  }
}

function formatResumeStatus(status: CodexSessionResumeStatus) {
  switch (status) {
    case "ready":
      return "可继续";
    case "running":
      return "占用中";
    case "missing_employee":
      return "缺少员工";
    case "missing_cli_session":
      return "不可恢复";
    case "stopping":
      return "停止中";
    case "invalid":
      return "无效";
    default:
      return status;
  }
}

function resumeBadgeVariant(status: CodexSessionResumeStatus): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "ready":
      return "default";
    case "running":
    case "stopping":
      return "secondary";
    case "missing_employee":
    case "missing_cli_session":
    case "invalid":
      return "destructive";
    default:
      return "outline";
  }
}

function buildLogTarget(session: {
  session_id?: string | null;
  resolved_session_id?: string | null;
  display_name?: string | null;
  employee_id?: string | null;
  employee_name?: string | null;
  task_id?: string | null;
  task_title?: string | null;
}): SessionLogTarget {
  return {
    sessionId: session.resolved_session_id ?? session.session_id ?? "未知",
    displayName: session.display_name ?? "未命名会话",
    employeeId: session.employee_id ?? null,
    employeeName: session.employee_name ?? null,
    taskId: session.task_id ?? null,
    taskTitle: session.task_title ?? null,
  };
}

export function SessionsPage() {
  const employees = useEmployeeStore((state) => state.employees);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const setCodexRunning = useEmployeeStore((state) => state.setCodexRunning);
  const refreshCodexRuntimeStatus = useEmployeeStore((state) => state.refreshCodexRuntimeStatus);

  const [sessions, setSessions] = useState<CodexSessionListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [page, setPage] = useState(1);
  const [continueDialogOpen, setContinueDialogOpen] = useState(false);
  const [continueSession, setContinueSession] = useState<CodexSessionListItem | null>(null);
  const [continueSubmitting, setContinueSubmitting] = useState(false);
  const [logDialogOpen, setLogDialogOpen] = useState(false);
  const [logTarget, setLogTarget] = useState<SessionLogTarget | null>(null);
  const [activeSession, setActiveSession] = useState<SessionLogTarget | null>(null);
  const [sessionIdQuery, setSessionIdQuery] = useState("");
  const [contentQuery, setContentQuery] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [infoMessage, setInfoMessage] = useState<string | null>(null);

  const filteredSessions = useMemo(() => {
    const normalizedSessionIdQuery = normalizeSearchText(sessionIdQuery);
    const normalizedContentQuery = normalizeSearchText(contentQuery);

    return sessions.filter((session) => {
      const matchesSessionId = !normalizedSessionIdQuery
        || normalizeSearchText(session.session_id).includes(normalizedSessionIdQuery)
        || normalizeSearchText(session.session_record_id).includes(normalizedSessionIdQuery)
        || normalizeSearchText(session.cli_session_id).includes(normalizedSessionIdQuery);

      if (!matchesSessionId) {
        return false;
      }

      if (!normalizedContentQuery) {
        return true;
      }

      const contentHaystack = [
        session.display_name,
        session.summary,
        session.content_preview,
        session.task_title,
        session.project_name,
        session.employee_name,
        session.working_dir,
      ]
        .map((value) => normalizeSearchText(value))
        .join("\n");

      return contentHaystack.includes(normalizedContentQuery);
    });
  }, [contentQuery, sessionIdQuery, sessions]);

  const totalPages = filteredSessions.length > 0 ? Math.ceil(filteredSessions.length / PAGE_SIZE) : 0;
  const pageSessions = useMemo(
    () => filteredSessions.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE),
    [filteredSessions, page],
  );
  const rangeStart = filteredSessions.length === 0 ? 0 : (page - 1) * PAGE_SIZE + 1;
  const rangeEnd = filteredSessions.length === 0 ? 0 : Math.min(page * PAGE_SIZE, filteredSessions.length);

  useEffect(() => {
    setPage(1);
  }, [contentQuery, sessionIdQuery]);

  useEffect(() => {
    if (totalPages > 0 && page > totalPages) {
      setPage(totalPages);
    }
    if (totalPages === 0 && page !== 1) {
      setPage(1);
    }
  }, [page, totalPages]);

  const loadSessions = async (silent = false) => {
    if (silent) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setErrorMessage(null);

    try {
      const [sessionItems] = await Promise.all([listCodexSessions(), fetchEmployees()]);
      setSessions(sessionItems);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "读取 Session 列表失败");
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void loadSessions();
  }, []);

  const openContinueDialog = (session: CodexSessionListItem) => {
    setContinueSession(session);
    setContinueDialogOpen(true);
    setErrorMessage(null);
    setInfoMessage(null);
  };

  const openLogDialog = (session: CodexSessionListItem) => {
    setLogTarget(buildLogTarget(session));
    setLogDialogOpen(true);
  };

  const handleContinueConversation = async (prompt: string) => {
    if (!continueSession) {
      return;
    }

    setContinueSubmitting(true);
    setErrorMessage(null);
    setInfoMessage(null);

    try {
      const preview = await prepareCodexSessionResume(continueSession.session_id);
      if (!preview.can_resume || !preview.resolved_session_id || !preview.employee_id) {
        setErrorMessage(preview.resume_message ?? "该 Session 当前不可继续对话");
        return;
      }

      const employee = employees.find((item) => item.id === preview.employee_id);
      await updateEmployeeStatus(preview.employee_id, "busy");
      setCodexRunning(preview.employee_id, true, preview.task_id ?? null);
      await startCodex(preview.employee_id, prompt, {
        model: employee?.model,
        reasoningEffort: employee?.reasoning_effort,
        systemPrompt: employee?.system_prompt,
        workingDir: preview.working_dir ?? undefined,
        taskId: preview.task_id ?? undefined,
        resumeSessionId: preview.resolved_session_id,
      });

      const nextLogTarget = buildLogTarget(preview);
      setActiveSession(nextLogTarget);
      setInfoMessage(`消息已发送到 Session ${preview.resolved_session_id}。`);
      setContinueDialogOpen(false);
      setContinueSession(null);
      setLogTarget(nextLogTarget);
      setLogDialogOpen(true);
      await loadSessions(true);
    } catch (error) {
      if (continueSession.employee_id) {
        setCodexRunning(continueSession.employee_id, false, null);
        await refreshCodexRuntimeStatus(continueSession.employee_id);
      }
      setErrorMessage(error instanceof Error ? error.message : "发送续聊消息失败");
    } finally {
      setContinueSubmitting(false);
    }
  };

  return (
    <>
      <div className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">Session 管理</h2>
            <p className="text-sm text-muted-foreground">
              使用表格分页查看最近 Session，并从操作列直接继续对话。
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            onClick={() => void loadSessions(true)}
            disabled={loading || refreshing}
          >
            {refreshing ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <RefreshCw className="mr-2 h-4 w-4" />}
            刷新
          </Button>
        </div>

        {activeSession && (
          <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-sm text-primary">
            当前最近一次续聊绑定到 Session <span className="font-mono">{activeSession.sessionId}</span>
            ，执行后会自动弹出终端日志。
          </div>
        )}

        {errorMessage && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {errorMessage}
          </div>
        )}

        {infoMessage && (
          <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-sm text-primary">
            {infoMessage}
          </div>
        )}

        <Card>
          <CardHeader>
            <CardTitle>Session 列表</CardTitle>
            <CardDescription>按最近更新时间倒序排列，支持 `session id` 搜索、内容搜索、分页查看与直接操作。</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground" htmlFor="session-id-search">
                  SessionId 搜索
                </label>
                <Input
                  id="session-id-search"
                  value={sessionIdQuery}
                  onChange={(event) => setSessionIdQuery(event.target.value)}
                  placeholder="输入 session id、session record id 或 CLI session id"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground" htmlFor="session-content-search">
                  内容搜索
                </label>
                <Input
                  id="session-content-search"
                  value={contentQuery}
                  onChange={(event) => setContentQuery(event.target.value)}
                  placeholder="搜索会话名称、摘要、最近事件内容、任务、项目、员工"
                />
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border/70">
              {loading ? (
                <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  正在加载 Session...
                </div>
              ) : filteredSessions.length === 0 ? (
                <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
                  没有符合当前搜索条件的 Session
                </div>
              ) : (
                <div className="overflow-x-auto">
                  <table className="min-w-full text-sm">
                    <thead className="bg-muted/40 text-left">
                      <tr className="border-b border-border">
                        <th className="px-4 py-3 font-medium">Session</th>
                        <th className="px-4 py-3 font-medium">状态</th>
                        <th className="px-4 py-3 font-medium">最近更新时间</th>
                        <th className="px-4 py-3 font-medium">关联任务</th>
                        <th className="px-4 py-3 font-medium">员工</th>
                        <th className="px-4 py-3 font-medium">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {pageSessions.map((session) => (
                        <tr key={session.session_record_id} className="border-b border-border/60 align-top last:border-b-0">
                          <td className="px-4 py-3">
                            <div className="space-y-1">
                              <div className="font-medium">{session.display_name}</div>
                              <div className="font-mono text-xs text-muted-foreground">{session.session_id}</div>
                              {session.summary && (
                                <div className="max-w-md text-xs text-muted-foreground">{session.summary}</div>
                              )}
                              {session.content_preview && (
                                <div className="max-w-md text-xs text-muted-foreground/80">
                                  内容：{session.content_preview}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="flex flex-col gap-2">
                              <Badge variant="outline">{formatSessionKind(session.session_kind)}</Badge>
                              <Badge variant="secondary">{formatSessionStatus(session.status)}</Badge>
                              <Badge variant={resumeBadgeVariant(session.resume_status)}>
                                {formatResumeStatus(session.resume_status)}
                              </Badge>
                            </div>
                          </td>
                          <td className="px-4 py-3 text-xs text-muted-foreground">
                            {new Date(session.last_updated_at).toLocaleString("zh-CN")}
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div>{session.task_title ?? "无关联任务"}</div>
                              <div className="text-muted-foreground">{session.project_name ?? "无关联项目"}</div>
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div>{session.employee_name ?? "未绑定"}</div>
                              {session.working_dir && (
                                <div className="max-w-56 break-all text-muted-foreground">{session.working_dir}</div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="flex min-w-40 flex-col gap-2">
                              <Button
                                type="button"
                                size="sm"
                                onClick={() => openContinueDialog(session)}
                                disabled={!session.can_resume}
                                title={session.resume_message ?? "继续对话"}
                              >
                                继续对话
                              </Button>
                              <Button
                                type="button"
                                size="sm"
                                variant="outline"
                                onClick={() => openLogDialog(session)}
                                disabled={!session.task_id && !session.employee_id}
                              >
                                查看日志
                              </Button>
                              {session.resume_message && (
                                <div className="text-xs text-muted-foreground">{session.resume_message}</div>
                              )}
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-muted-foreground">
                {filteredSessions.length === 0
                  ? "暂无分页数据"
                  : `显示 ${rangeStart}-${rangeEnd} 条，共 ${filteredSessions.length} 条`}
              </span>
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">
                  {filteredSessions.length === 0 ? "第 0 / 0 页" : `第 ${page} / ${totalPages} 页`}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setPage((current) => Math.max(1, current - 1))}
                  disabled={loading || page <= 1}
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                  上一页
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setPage((current) => current + 1)}
                  disabled={loading || filteredSessions.length === 0 || page >= totalPages}
                >
                  下一页
                  <ChevronRight className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <SessionContinueDialog
        open={continueDialogOpen}
        session={continueSession}
        submitting={continueSubmitting}
        onOpenChange={(open) => {
          if (!continueSubmitting) {
            setContinueDialogOpen(open);
            if (!open) {
              setContinueSession(null);
            }
          }
        }}
        onConfirm={handleContinueConversation}
      />

      <SessionLogDialog
        open={logDialogOpen}
        session={logTarget}
        onOpenChange={setLogDialogOpen}
      />
    </>
  );
}
