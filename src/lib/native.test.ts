import { describe, expect, it } from "vitest";

import { nativeKToTokens, nativeTokensToK, normalizeNativeTokenK } from "@/lib/native";

describe("native token K units", () => {
  it("converts persisted token counts to readable K values", () => {
    expect(nativeTokensToK(256_000)).toBe(256);
    expect(nativeTokensToK(200_000)).toBe(200);
    expect(nativeTokensToK(4_096)).toBe(4.096);
  });

  it("converts K input values back to rounded tokens", () => {
    expect(nativeKToTokens(256)).toBe(256_000);
    expect(nativeKToTokens(4.096)).toBe(4_096);
    expect(nativeKToTokens(4.0965)).toBe(4_097);
  });

  it("bounds K values using the persisted token limits", () => {
    expect(normalizeNativeTokenK(1, 8_000, 1_000_000, 16_000)).toBe(8);
    expect(normalizeNativeTokenK(2_000, 8_000, 1_000_000, 16_000)).toBe(1_000);
    expect(normalizeNativeTokenK(Number.NaN, 8_000, 1_000_000, 16_000)).toBe(16);
  });
});
