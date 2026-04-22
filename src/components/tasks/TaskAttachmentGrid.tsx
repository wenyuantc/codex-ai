import { useEffect, useMemo, useState } from "react";
import { ExternalLink, File, Image as ImageIcon, Trash2 } from "lucide-react";

import { readImageFile } from "@/lib/backend";
import {
  formatAttachmentFileSize,
  guessImageMimeType,
  isImageAttachment,
  isTauriRuntime,
} from "@/lib/taskAttachments";

interface TaskAttachmentGridItem {
  id: string;
  name: string;
  path: string;
  fileSize?: number;
  mimeType?: string;
  removable?: boolean;
  onRemove?: () => void;
  onOpen?: () => void;
}

interface TaskAttachmentGridProps {
  items: TaskAttachmentGridItem[];
  emptyText?: string;
}

export function TaskAttachmentGrid({
  items,
  emptyText = "暂无附件",
}: TaskAttachmentGridProps) {
  const [previewUrls, setPreviewUrls] = useState<Record<string, string>>({});
  const previewKey = useMemo(
    () => items.map((item) => `${item.id}:${item.path}:${item.mimeType ?? ""}`).join("|"),
    [items],
  );
  const previewItems = useMemo(
    () => items
      .filter((item) => isImageAttachment(item.path, item.mimeType))
      .map((item) => ({
        id: item.id,
        path: item.path,
        mimeType: item.mimeType,
      })),
    [previewKey],
  );

  useEffect(() => {
    let disposed = false;
    const objectUrls: string[] = [];

    async function loadPreviews() {
      if (!isTauriRuntime() || previewItems.length === 0) {
        setPreviewUrls((current) => (Object.keys(current).length === 0 ? current : {}));
        return;
      }

      const results = await Promise.all(
        previewItems.map(async (item) => {
          try {
            const bytes = await readImageFile(item.path);
            const blob = new Blob(
              [new Uint8Array(bytes)],
              { type: item.mimeType || guessImageMimeType(item.path) },
            );
            const url = URL.createObjectURL(blob);
            objectUrls.push(url);
            return [item.id, url] as const;
          } catch (error) {
            console.error("Failed to load attachment preview:", error);
            return [item.id, ""] as const;
          }
        }),
      );

      if (disposed) {
        objectUrls.forEach((url) => URL.revokeObjectURL(url));
        return;
      }

      setPreviewUrls(
        Object.fromEntries(results.filter(([, url]) => url)),
      );
    }

    void loadPreviews();

    return () => {
      disposed = true;
      objectUrls.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [previewItems]);

  if (items.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
        {emptyText}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4">
      {items.map((item) => {
        const previewSrc = previewUrls[item.id];
        const isImage = isImageAttachment(item.path, item.mimeType);

        return (
          <div
            key={item.id}
            className="overflow-hidden rounded-lg border border-border bg-card"
          >
            <div className="relative aspect-[4/3] bg-muted/40">
              {previewSrc ? (
                <img
                  src={previewSrc}
                  alt={item.name}
                  className="h-full w-full object-cover"
                />
              ) : (
                <div className="flex h-full items-center justify-center text-muted-foreground">
                  {isImage ? <ImageIcon className="h-8 w-8" /> : <File className="h-8 w-8" />}
                </div>
              )}

              <div className="absolute right-2 top-2 flex items-center gap-1">
                {item.onOpen && (
                  <button
                    type="button"
                    onClick={item.onOpen}
                    className="rounded-full bg-black/65 p-1.5 text-white transition-colors hover:bg-black/80"
                    title="打开附件"
                  >
                    <ExternalLink className="h-3.5 w-3.5" />
                  </button>
                )}
                {item.removable && item.onRemove && (
                  <button
                    type="button"
                    onClick={item.onRemove}
                    className="rounded-full bg-black/65 p-1.5 text-white transition-colors hover:bg-red-600"
                    title="移除附件"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            </div>

            <div className="space-y-1 px-3 py-2">
              <p className="truncate text-xs font-medium text-foreground" title={item.name}>
                {item.name}
              </p>
              {typeof item.fileSize === "number" && (
                <p className="text-[11px] text-muted-foreground">
                  {formatAttachmentFileSize(item.fileSize)}
                </p>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
