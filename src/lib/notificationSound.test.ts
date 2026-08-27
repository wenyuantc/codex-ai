import { describe, expect, it } from "vitest";

import type { DesktopNotificationEvent } from "@/lib/types";
import {
  buildNotificationBeepWavBytes,
  buildNotificationSoundDeliveryKey,
  parseNotificationSoundEnabled,
  shouldPlayNotificationSound,
} from "@/lib/notificationSound";

function deliveryEvent(
  overrides: Partial<DesktopNotificationEvent> = {},
): DesktopNotificationEvent {
  return {
    reason: "created",
    notification_id: "note-1",
    title: "title",
    message: "message",
    severity: "warning",
    action_route: null,
    project_id: null,
    task_id: null,
    ssh_config_id: null,
    is_transient: false,
    last_triggered_at: "2026-08-27 12:00:00",
    ...overrides,
  };
}

describe("parseNotificationSoundEnabled", () => {
  it("defaults to enabled when unset or unrecognized", () => {
    expect(parseNotificationSoundEnabled(null)).toBe(true);
    expect(parseNotificationSoundEnabled("true")).toBe(true);
    expect(parseNotificationSoundEnabled("")).toBe(true);
    expect(parseNotificationSoundEnabled("maybe")).toBe(true);
  });

  it("treats false as disabled", () => {
    expect(parseNotificationSoundEnabled("false")).toBe(false);
  });
});

describe("buildNotificationSoundDeliveryKey", () => {
  it("uses reason, id, and timestamp for persisted notifications", () => {
    expect(buildNotificationSoundDeliveryKey(deliveryEvent())).toBe(
      "created:note-1:2026-08-27 12:00:00",
    );
  });

  it("uses the transient prefix for transient deliveries", () => {
    expect(
      buildNotificationSoundDeliveryKey(
        deliveryEvent({
          is_transient: true,
          notification_id: "transient:sdk",
          reason: "transient",
        }),
      ),
    ).toBe("transient:transient:sdk");
  });
});

describe("buildNotificationBeepWavBytes", () => {
  it("writes a PCM wav header", () => {
    const wav = buildNotificationBeepWavBytes();
    const header = String.fromCharCode(...wav.slice(0, 12));
    expect(header.startsWith("RIFF")).toBe(true);
    expect(header.endsWith("WAVE")).toBe(true);
    expect(wav.byteLength).toBeGreaterThan(40_000);
  });
});

describe("shouldPlayNotificationSound", () => {
  it("plays previews even when disabled", () => {
    expect(shouldPlayNotificationSound(false, true)).toBe(true);
    expect(shouldPlayNotificationSound(true, true)).toBe(true);
  });

  it("plays real deliveries only when enabled", () => {
    expect(shouldPlayNotificationSound(true, false)).toBe(true);
    expect(shouldPlayNotificationSound(false, false)).toBe(false);
  });
});
