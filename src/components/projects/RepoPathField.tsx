import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, X } from "lucide-react";
import { useTranslation } from "react-i18next";

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
  typeof window !== "undefined" &&
  typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
    "undefined";

export function RepoPathField({
  label,
  placeholder,
  value,
  onChange,
  onDirectorySelected,
}: RepoPathFieldProps) {
  const { t } = useTranslation("projects");
  const handleSelectDirectory = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: value.trim() || undefined,
      title: t("repoSelectFolderTitle"),
    });

    if (typeof selected === "string") {
      onChange(selected);
      onDirectorySelected?.(selected);
    }
  };

  const resolvedLabel = label ?? t("repoPathLabel");
  const resolvedPlaceholder = placeholder ?? t("repoPathPlaceholder");

  return (
    <div>
      <label className="text-xs font-medium text-muted-foreground">{resolvedLabel}</label>
      <div className="mt-1 flex gap-2">
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={resolvedPlaceholder}
          className="flex-1"
        />
        <Button
          type="button"
          variant="outline"
          onClick={handleSelectDirectory}
          disabled={!isTauriRuntime}
          title={isTauriRuntime ? t("repoSelectTitle") : t("repoSelectDesktopOnly")}
        >
          <FolderOpen className="h-4 w-4" />
          {t("repoSelect")}
        </Button>
        {value.trim() && (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => onChange("")}
            title={t("repoClearPath")}
          >
            <X className="h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
