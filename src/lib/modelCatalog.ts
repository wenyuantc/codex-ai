import type { AiChannelModel, ModelCatalogEntry } from "@/lib/types";

export function emptyChannelModel(id = ""): AiChannelModel {
  return {
    id,
    context_tokens: null,
    max_output_tokens: null,
    thinking_enabled: null,
    thinking_level: null,
    thinking_levels: null,
  };
}

export function normalizeModelKey(value: string): string {
  const trimmed = value.trim();
  const last = trimmed.split(/[/:]/).pop() ?? trimmed;
  return last.replace(/_/g, "-").toLowerCase();
}

export function lookupModelCatalog(
  catalog: ModelCatalogEntry[],
  modelId: string,
): ModelCatalogEntry | null {
  const raw = modelId.trim();
  if (!raw) return null;
  const key = normalizeModelKey(raw);
  const exact = catalog.find(
    (entry) =>
      entry.id.toLowerCase() === raw.toLowerCase() ||
      entry.aliases.some((alias) => alias.toLowerCase() === raw.toLowerCase()),
  );
  if (exact) return exact;
  const exactKey = catalog.find(
    (entry) =>
      normalizeModelKey(entry.id) === key ||
      entry.aliases.some((alias) => normalizeModelKey(alias) === key),
  );
  if (exactKey) return exactKey;
  const prefixMatches = catalog.filter((entry) => {
    const catalogKey = normalizeModelKey(entry.id);
    return catalogKey.length >= 6 && (key.startsWith(catalogKey) || catalogKey.startsWith(key));
  });
  if (prefixMatches.length === 0) return null;
  return prefixMatches.reduce((best, entry) =>
    normalizeModelKey(entry.id).length > normalizeModelKey(best.id).length ? entry : best,
  );
}

export function applyCatalogToModel(
  catalog: ModelCatalogEntry[],
  model: AiChannelModel,
  overwrite = false,
): AiChannelModel {
  const entry = lookupModelCatalog(catalog, model.id);
  if (!entry) return { ...model, id: model.id.trim() };
  const next: AiChannelModel = { ...model, id: model.id.trim() };
  if (overwrite || next.context_tokens == null) next.context_tokens = entry.context_tokens;
  if (overwrite || next.max_output_tokens == null) next.max_output_tokens = entry.max_output_tokens;
  if (overwrite || next.thinking_enabled == null) next.thinking_enabled = entry.thinking;
  if (overwrite || !next.thinking_level) {
    next.thinking_level = entry.thinking
      ? (entry.thinking_levels.find((level) => level === "medium") ??
        entry.thinking_levels[0] ??
        null)
      : null;
  }
  const storedLevels = next.thinking_levels ?? [];
  const catalogHasNewLevel = entry.thinking_levels.some((level) => !storedLevels.includes(level));
  if (overwrite || storedLevels.length === 0 || catalogHasNewLevel) {
    next.thinking_levels = entry.thinking_levels.length > 0 ? entry.thinking_levels : [];
  }
  if (!entry.thinking && (overwrite || model.thinking_enabled == null)) {
    next.thinking_enabled = false;
    if (overwrite || !model.thinking_level) next.thinking_level = null;
    if (overwrite || !model.thinking_levels?.length) next.thinking_levels = [];
  }
  return next;
}

export function channelModelIds(models: AiChannelModel[]): string[] {
  return models.map((item) => item.id).filter((id) => id.trim().length > 0);
}
