import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface RepoPathFieldProps {
  label?: string;
  placeholder?: string;
  value: string;
  onChange: (value: string) => void;
  onDirectorySelected?: (path: string) => void;
}

const isTauriRuntime =
  typeof window !== "undefined" && typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

export function RepoPathField({
  label = "仓库路径",
  placeholder = "/path/to/repo（可选）",
  value,
  onChange,
  onDirectorySelected,
}: RepoPathFieldProps) {
  const handleSelectDirectory = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: value.trim() || undefined,
      title: "选择仓库文件夹",
    });

    if (typeof selected === "string") {
      onChange(selected);
      onDirectorySelected?.(selected);
    }
  };

  return (
    <div>
      <label className="text-xs font-medium text-muted-foreground">{label}</label>
      <div className="mt-1 flex gap-2">
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className="flex-1"
        />
        <Button
          type="button"
          variant="outline"
          onClick={handleSelectDirectory}
          disabled={!isTauriRuntime}
          title={isTauriRuntime ? "选择文件夹" : "仅桌面端支持选择文件夹"}
        >
          <FolderOpen className="h-4 w-4" />
          选择
        </Button>
        {value.trim() && (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => onChange("")}
            title="清空路径"
          >
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
