export type NativeSubagentScope = "all" | "projects";

export function nativeSubagentMatchesProject(
  item: { scope?: string | null; project_ids?: string[] | null },
  projectId?: string | null,
): boolean {
  const scope = item.scope === "projects" ? "projects" : "all";
  if (scope === "all") {
    return true;
  }
  const id = projectId?.trim();
  if (!id) {
    return false;
  }
  return (item.project_ids ?? []).includes(id);
}
