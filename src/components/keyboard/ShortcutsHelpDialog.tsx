import { useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Kbd } from "@/components/keyboard/Kbd";
import { getAllShortcuts, shortcutDisplay, type ShortcutDef } from "@/lib/shortcuts";

const categoryLabels: Record<string, string> = {
  navigation: "页面导航",
  global: "全局操作",
  page: "页面操作",
};

function ShortcutRow({ shortcut }: { shortcut: ShortcutDef }) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-sm text-foreground">{shortcut.description}</span>
      <Kbd variant="outline" size="sm">{shortcutDisplay(shortcut)}</Kbd>
    </div>
  );
}

function ShortcutsSection({ category, shortcuts }: { category: string; shortcuts: ShortcutDef[] }) {
  return (
    <div>
      <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1">
        {categoryLabels[category] || category}
      </h3>
      <div className="divide-y divide-border/50">
        {shortcuts.map((s) => (
          <ShortcutRow key={s.id} shortcut={s} />
        ))}
      </div>
    </div>
  );
}

function groupedShortcuts() {
  const all = getAllShortcuts();
  return {
    navigation: all.filter((s) => s.category === "navigation"),
    global: all.filter((s) => s.category === "global"),
    page: all.filter((s) => s.category === "page"),
  };
}

export function ShortcutsHelpDialog() {
  const [open, setOpen] = useState(false);

  useHotkeys("?", (e) => {
    e.preventDefault();
    setOpen(true);
  });

  const groups = groupedShortcuts();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>键盘快捷键</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <ShortcutsSection category="navigation" shortcuts={groups.navigation} />
          <ShortcutsSection category="global" shortcuts={groups.global} />
          <ShortcutsSection category="page" shortcuts={groups.page} />
        </div>
      </DialogContent>
    </Dialog>
  );
}
