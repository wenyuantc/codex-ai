import { describe, expect, it } from "vitest";

import { resolveImageAttachmentSkip } from "@/lib/imageAttachmentSkip";

describe("resolveImageAttachmentSkip", () => {
  it("does not warn when there are no images", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 0,
        provider: "grok",
        projectType: "ssh",
      }),
    ).toBeNull();
  });

  it("does not warn for local Codex or OpenCode", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 2,
        provider: "codex",
        projectType: "local",
      }),
    ).toBeNull();
    expect(
      resolveImageAttachmentSkip({
        imageCount: 2,
        provider: "opencode",
        projectType: "local",
        hasTaskId: true,
      }),
    ).toBeNull();
  });

  it("does not warn for SSH Codex/OpenCode when the task can sync attachments", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "codex",
        projectType: "ssh",
        hasTaskId: true,
      }),
    ).toBeNull();
  });

  it("warns for Grok SSH and Claude SSH, but not local Claude CLI", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "grok",
        projectType: "ssh",
      }),
    ).toBe("ssh_grok");
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "claude",
        projectType: "ssh",
        claudeEffectiveProvider: "sdk",
      }),
    ).toBe("ssh_claude");
    expect(
      resolveImageAttachmentSkip({
        imageCount: 3,
        provider: "claude",
        projectType: "local",
        claudeEffectiveProvider: "cli",
      }),
    ).toBeNull();
  });

  it("does not warn for built-in Agent images", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "native",
        projectType: "local",
      }),
    ).toBeNull();
    expect(
      resolveImageAttachmentSkip({
        imageCount: 2,
        provider: "native",
        projectType: "ssh",
        hasTaskId: true,
      }),
    ).toBeNull();
  });

  it("warns for SSH Codex without a task id", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "codex",
        projectType: "ssh",
        hasTaskId: false,
      }),
    ).toBe("ssh_no_task");
  });

  it("does not warn for local Claude SDK", () => {
    expect(
      resolveImageAttachmentSkip({
        imageCount: 1,
        provider: "claude",
        projectType: "local",
        claudeEffectiveProvider: "sdk",
      }),
    ).toBeNull();
  });
});
