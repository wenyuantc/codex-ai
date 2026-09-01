import { useEffect, useState } from "react";
import { FolderOpen, Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { listNativeGlobalSkills, openNativeSkillsDir, type NativeSkill } from "@/lib/native";

export function NativeSkillsSettingsCard() {
  const { t } = useTranslation("settings");
  const [dir, setDir] = useState("");
  const [skills, setSkills] = useState<NativeSkill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listNativeGlobalSkills();
      setDir(result.dir);
      setSkills(result.skills);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  return (
    <div className="space-y-3 rounded-lg border border-border bg-card p-4">
      <div>
        <h3 className="text-sm font-medium">{t("nativeSkills.title")}</h3>
        <p className="text-xs text-muted-foreground">{t("nativeSkills.description")}</p>
      </div>
      <p className="break-all font-mono text-xs text-muted-foreground">
        {dir || t("nativeSkills.dirUnknown")}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            void openNativeSkillsDir().catch((err) => {
              setError(err instanceof Error ? err.message : String(err));
            });
          }}
        >
          <FolderOpen className="h-4 w-4" />
          {t("nativeSkills.openDir")}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void load()} disabled={loading}>
          {loading ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
          {t("nativeSkills.refresh")}
        </Button>
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
      {skills.length === 0 && !loading && (
        <p className="text-sm text-muted-foreground">{t("nativeSkills.empty")}</p>
      )}
      {skills.length > 0 && (
        <ul className="space-y-2">
          {skills.map((skill) => (
            <li
              key={`${skill.source}-${skill.name}`}
              className="rounded-md border border-border px-3 py-2"
            >
              <p className="text-sm font-medium">{skill.name}</p>
              <p className="text-xs text-muted-foreground">{skill.description}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
