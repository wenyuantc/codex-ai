import { describe, expect, it } from "vitest";

import { applyCatalogToModel, emptyChannelModel, lookupModelCatalog } from "@/lib/modelCatalog";
import type { ModelCatalogEntry } from "@/lib/types";

const catalog: ModelCatalogEntry[] = [
  {
    id: "gpt-4o",
    aliases: ["chatgpt-4o-latest"],
    vendor: "openai",
    label: "GPT-4o",
    context_tokens: 128000,
    max_output_tokens: 16384,
    thinking: false,
    thinking_levels: [],
  },
  {
    id: "deepseek-reasoner",
    aliases: ["deepseek-r1"],
    vendor: "deepseek",
    label: "DeepSeek Reasoner",
    context_tokens: 163840,
    max_output_tokens: 65536,
    thinking: true,
    thinking_levels: ["low", "medium", "high"],
  },
];

describe("model catalog lookup", () => {
  it("matches ids, aliases, and prefixed gateway ids", () => {
    expect(lookupModelCatalog(catalog, "gpt-4o")?.id).toBe("gpt-4o");
    expect(lookupModelCatalog(catalog, "chatgpt-4o-latest")?.id).toBe("gpt-4o");
    expect(lookupModelCatalog(catalog, "deepseek-ai/deepseek-r1")?.id).toBe("deepseek-reasoner");
  });

  it("fills metadata for a new model id", () => {
    const filled = applyCatalogToModel(catalog, emptyChannelModel("deepseek-reasoner"));
    expect(filled.context_tokens).toBe(163840);
    expect(filled.max_output_tokens).toBe(65536);
    expect(filled.thinking_enabled).toBe(true);
    expect(filled.thinking_level).toBe("medium");
    expect(filled.thinking_levels).toEqual(["low", "medium", "high"]);
  });
});
