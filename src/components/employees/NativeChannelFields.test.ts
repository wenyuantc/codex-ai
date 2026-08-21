import { describe, expect, it } from "vitest";

import { resolveNativeThinking } from "./NativeChannelFields";
import type { AiChannel } from "@/lib/types";

function channel(models: AiChannel["models"]): AiChannel {
  return {
    id: "ch1",
    name: "demo",
    protocol: "openai",
    base_url: "https://example.com",
    extra_headers_json: null,
    models,
    enabled: true,
    api_key: "sk-test",
    api_key_configured: true,
    created_at: "",
    updated_at: "",
  };
}

describe("resolveNativeThinking", () => {
  it("uses the channel model thinking level list", () => {
    const result = resolveNativeThinking(
      channel([
        {
          id: "gpt-5.6-luna",
          context_tokens: 1000000,
          max_output_tokens: 128000,
          thinking_enabled: true,
          thinking_level: "medium",
          thinking_levels: ["minimal", "low", "medium", "high"],
        },
      ]),
      "gpt-5.6-luna",
    );
    expect(result.enabled).toBe(true);
    expect(result.levels).toEqual(["minimal", "low", "medium", "high"]);
    expect(result.defaultLevel).toBe("medium");
  });

  it("disables thinking when the channel model turned it off", () => {
    const result = resolveNativeThinking(
      channel([
        {
          id: "gpt-4o",
          context_tokens: 128000,
          max_output_tokens: 16384,
          thinking_enabled: false,
          thinking_level: null,
          thinking_levels: [],
        },
      ]),
      "gpt-4o",
    );
    expect(result.enabled).toBe(false);
  });
});
