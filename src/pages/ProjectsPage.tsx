import { useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useTranslation } from "react-i18next";
import { Kbd } from "@/components/keyboard/Kbd";
import { ProjectList } from "@/components/projects/ProjectList";
import { CreateProjectDialog } from "@/components/projects/CreateProjectDialog";
import { useProjectStore } from "@/stores/projectStore";
import { Plus } from "lucide-react";

export function ProjectsPage() {
  const { t } = useTranslation("projects");
  const [showCreate, setShowCreate] = useState(false);
  const environmentMode = useProjectStore((state) => state.environmentMode);

  useHotkeys("n", () => setShowCreate(true), { preventDefault: true });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">{t("listTitle")}</h2>
          <p className="text-sm text-muted-foreground">
            {environmentMode === "ssh" ? t("scopeSsh") : t("scopeLocal")}
          </p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          {t("create")}
          <Kbd variant="primary" size="xs" className="ml-1.5">
            N
          </Kbd>
        </button>
      </div>

      <ProjectList onCreateProject={() => setShowCreate(true)} />
      <CreateProjectDialog open={showCreate} onOpenChange={setShowCreate} />
    </div>
  );
}
