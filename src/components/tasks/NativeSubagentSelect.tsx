import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { listNativeSubagents, type NativeSubagent } from "@/lib/native";
import { nativeSubagentMatchesProject } from "@/lib/nativeSubagentScope";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const NONE_VALUE = "__none__";

interface NativeSubagentSelectProps {
  value: string;
  projectId?: string;
  clearIfOutOfScope?: boolean;
  disabled?: boolean;
  triggerClassName?: string;
  onChange: (value: string) => void;
}

export function NativeSubagentSelect({
  value,
  projectId,
  clearIfOutOfScope = false,
  disabled = false,
  triggerClassName,
  onChange,
}: NativeSubagentSelectProps) {
  const { t } = useTranslation("tasks");
  const [items, setItems] = useState<NativeSubagent[]>([]);

  useEffect(() => {
    void listNativeSubagents()
      .then(setItems)
      .catch(() => setItems([]));
  }, []);

  const visibleItems = useMemo(() => {
    return items.filter(
      (item) => item.id === value || nativeSubagentMatchesProject(item, projectId),
    );
  }, [items, projectId, value]);

  useEffect(() => {
    if (!clearIfOutOfScope || !value) {
      return;
    }
    const selectedItem = items.find((item) => item.id === value);
    if (!selectedItem) {
      return;
    }
    if (!nativeSubagentMatchesProject(selectedItem, projectId)) {
      onChange("");
    }
    // Parent onChange identity is not stable; only re-run when selection/project changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see above
  }, [clearIfOutOfScope, items, projectId, value]);

  const selected = visibleItems.find((item) => item.id === value);

  return (
    <Select
      disabled={disabled}
      value={value || NONE_VALUE}
      onValueChange={(next) => onChange(!next || next === NONE_VALUE ? "" : String(next))}
    >
      <SelectTrigger className={triggerClassName ?? "mt-1 bg-background"}>
        <SelectValue>
          {(current) => {
            if (!current || current === NONE_VALUE) {
              return t("nativeSubagent.unspecified");
            }
            return selected?.name ?? t("nativeSubagent.missing");
          }}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={NONE_VALUE}>{t("nativeSubagent.unspecified")}</SelectItem>
        {visibleItems.map((item) => (
          <SelectItem key={item.id} value={item.id}>
            {item.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
