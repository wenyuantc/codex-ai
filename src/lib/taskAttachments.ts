import { isTauri } from "@tauri-apps/api/core";

export const IMAGE_FILE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];

export const IMAGE_FILE_FILTERS = [
  {
    name: "Images",
    extensions: IMAGE_FILE_EXTENSIONS,
  },
];

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && isTauri();
}

export function normalizeDialogSelection(selected: string | string[] | null): string[] {
  if (Array.isArray(selected)) {
    return selected;
  }

  return typeof selected === "string" ? [selected] : [];
}

export function dedupePaths(paths: string[]): string[] {
  return Array.from(
    new Set(
      paths
        .map((path) => path.trim())
        .filter((path) => path.length > 0),
    ),
  );
}

export function guessImageMimeType(path: string): string {
  const extension = path.split(".").pop()?.toLowerCase();
  switch (extension) {
    case "png":
      return "image/png";
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "bmp":
      return "image/bmp";
    case "svg":
      return "image/svg+xml";
    default:
      return "application/octet-stream";
  }
}

export function isImageMimeType(mimeType?: string | null): boolean {
  return typeof mimeType === "string" && mimeType.toLowerCase().startsWith("image/");
}

export function isImageAttachment(path: string, mimeType?: string | null): boolean {
  if (isImageMimeType(mimeType)) {
    return true;
  }

  const extension = path.split(".").pop()?.toLowerCase();
  return Boolean(extension && IMAGE_FILE_EXTENSIONS.includes(extension));
}

export function formatAttachmentFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
