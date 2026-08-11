import type { CodexSessionFileChangeDetail } from "@/lib/types";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  getExecutionChangeCaptureModeDescription,
  getExecutionChangeCaptureModeLabel,
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getExecutionDiffLineClassName,
  getExecutionSnapshotStatusLabel,
} from "./taskDetailViewHelpers";

interface TaskExecutionChangeDetailDialogProps {
  open: boolean;
  loading: boolean;
  error: string | null;
  detail: CodexSessionFileChangeDetail | null;
  onOpenChange: (open: boolean) => void;
}

function CodePreview({ text, diffMode = false }: { text: string; diffMode?: boolean }) {
  const lines = text.split(/\r?\n/);

  return (
    <ScrollArea className="h-[28rem] overflow-hidden rounded-md border bg-background/80">
      <div className="p-3 font-mono text-xs leading-5">
        {lines.map((line, index) => (
          <div
            key={`${index}-${line}`}
            className={`whitespace-pre-wrap break-all ${diffMode ? getExecutionDiffLineClassName(line) : "text-foreground"}`}
          >
            {line || " "}
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}

function SnapshotMeta({
  label,
  status,
  truncated,
}: {
  label: string;
  status: "text" | "missing" | "binary" | "unavailable";
  truncated: boolean;
}) {
  const { t } = useTranslation("tasks");
  return (
    <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs">
      <div className="font-medium text-foreground">{label}</div>
      <div className="mt-1 text-muted-foreground">
        {getExecutionSnapshotStatusLabel(status)}
        {truncated ? ` ${t("detail.labels.truncatedSuffix")}` : ""}
      </div>
    </div>
  );
}

export function TaskExecutionChangeDetailDialog({
  open,
  loading,
  error,
  detail,
  onOpenChange,
}: TaskExecutionChangeDetailDialogProps) {
  const { t } = useTranslation("tasks");
  const hasBeforeText = detail?.before_status === "text" && detail.before_text !== null;
  const hasAfterText = detail?.after_status === "text" && detail.after_text !== null;
  const hasDiffText = detail?.diff_text !== null && detail?.diff_text !== undefined;
  const defaultTab =
    detail?.change.change_type === "added" && hasAfterText
      ? "after"
      : hasDiffText
        ? "diff"
        : hasBeforeText
          ? "before"
          : "after";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,72rem)] max-w-[min(96vw,72rem)] sm:max-w-[min(96vw,72rem)]">
        <DialogHeader>
          <DialogTitle>
            {detail
              ? `${getExecutionChangeTypeLabel(detail.change.change_type)} ${detail.change.path}`
              : t("detail.changeDetail.titleFallback")}
          </DialogTitle>
          <DialogDescription>{t("detail.changeDetail.description")}</DialogDescription>
        </DialogHeader>

        {loading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            {t("detail.changeDetail.loading")}
          </div>
        ) : error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive">
            {error}
          </div>
        ) : !detail ? (
          <div className="rounded-md border border-dashed border-border px-3 py-8 text-center text-sm text-muted-foreground">
            {t("detail.changeDetail.empty")}
          </div>
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={`rounded-md border px-2 py-1 text-xs font-medium ${getExecutionChangeTypeClassName(detail.change.change_type)}`}
              >
                {getExecutionChangeTypeLabel(detail.change.change_type)}
              </span>
              <span className="rounded-md border border-border/70 bg-muted/30 px-2 py-1 text-xs text-muted-foreground">
                {getExecutionChangeCaptureModeLabel(detail.change.capture_mode)}
              </span>
            </div>

            <div className="rounded-md border border-border/70 bg-muted/20 p-3 text-xs text-muted-foreground">
              {getExecutionChangeCaptureModeDescription(detail.change.capture_mode)}
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">
                  {t("detail.changeDetail.absolutePath")}
                </div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.absolute_path ?? t("detail.labels.notRecorded")}
                </div>
              </div>
              <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs">
                <div className="font-medium text-foreground">
                  {t("detail.changeDetail.workingDir")}
                </div>
                <div className="mt-1 break-all font-mono text-muted-foreground">
                  {detail.working_dir ?? t("detail.labels.notRecorded")}
                </div>
              </div>
              {detail.change.previous_path && (
                <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-xs md:col-span-2">
                  <div className="font-medium text-foreground">
                    {t("detail.changeDetail.previousPath")}
                  </div>
                  <div className="mt-1 whitespace-pre-wrap break-all font-mono text-muted-foreground">
                    {detail.change.previous_path}
                    {detail.previous_absolute_path ? `\n${detail.previous_absolute_path}` : ""}
                  </div>
                </div>
              )}
            </div>

            {detail.snapshot_message && (
              <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
                {detail.snapshot_message}
              </div>
            )}

            {detail.snapshot_status === "ready" && (
              <>
                <div className="grid gap-2 md:grid-cols-2">
                  <SnapshotMeta
                    label={t("detail.changeDetail.beforeSnapshot")}
                    status={detail.before_status}
                    truncated={detail.before_truncated}
                  />
                  <SnapshotMeta
                    label={t("detail.changeDetail.afterSnapshot")}
                    status={detail.after_status}
                    truncated={detail.after_truncated}
                  />
                </div>

                {detail.diff_truncated && (
                  <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                    {t("detail.changeDetail.diffTruncated")}
                  </div>
                )}

                {hasDiffText || hasBeforeText || hasAfterText ? (
                  <Tabs defaultValue={defaultTab}>
                    <TabsList className="grid w-full grid-cols-3">
                      <TabsTrigger value="diff" disabled={!hasDiffText}>
                        {t("detail.changeDetail.tabDiff")}
                      </TabsTrigger>
                      <TabsTrigger value="before" disabled={!hasBeforeText}>
                        {t("detail.changeDetail.tabBefore")}
                      </TabsTrigger>
                      <TabsTrigger value="after" disabled={!hasAfterText}>
                        {t("detail.changeDetail.tabAfter")}
                      </TabsTrigger>
                    </TabsList>

                    <TabsContent value="diff" className="space-y-2">
                      {hasDiffText ? (
                        <CodePreview text={detail.diff_text ?? ""} diffMode />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          {t("detail.changeDetail.noDiff")}
                        </div>
                      )}
                    </TabsContent>

                    <TabsContent value="before" className="space-y-2">
                      {hasBeforeText ? (
                        <CodePreview text={detail.before_text ?? ""} />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          {t("detail.changeDetail.noBefore")}
                        </div>
                      )}
                    </TabsContent>

                    <TabsContent value="after" className="space-y-2">
                      {hasAfterText ? (
                        <CodePreview text={detail.after_text ?? ""} />
                      ) : (
                        <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                          {t("detail.changeDetail.noAfter")}
                        </div>
                      )}
                    </TabsContent>
                  </Tabs>
                ) : (
                  <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                    {t("detail.changeDetail.noTextContent")}
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
