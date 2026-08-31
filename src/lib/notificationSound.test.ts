import { describe, expect, it } from "vitest";

import type { DesktopNotificationEvent } from "@/lib/types";
import {
  buildNotificationBeepWavBytes,
  buildNotificationSoundDeliveryKey,
  NOTIFICATION_FALLBACK_BEEP_DURATION_SECONDS,
  NOTIFICATION_SOUND_COALESCE_MS,
  parseNotificationSoundEnabled,
  shouldPlayCoalescedSound,
  shouldPlayDeliveredNotificationSound,
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
  it("writes a short PCM wav header", () => {
    const wav = buildNotificationBeepWavBytes();
    const header = String.fromCharCode(...wav.slice(0, 12));
    expect(header.startsWith("RIFF")).toBe(true);
    expect(header.endsWith("WAVE")).toBe(true);
    expect(NOTIFICATION_FALLBACK_BEEP_DURATION_SECONDS).toBeLessThanOrEqual(0.25);
    expect(wav.byteLength).toBeGreaterThan(2_000);
    expect(wav.byteLength).toBeLessThan(12_000);
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

describe("shouldPlayDeliveredNotificationSound", () => {
  it("drops startup deliveries even when enabled", () => {
    expect(
      shouldPlayDeliveredNotificationSound({
        enabled: true,
        preferenceReady: true,
        startupSuppressed: true,
      }),
    ).toBe(false);
  });

  it("drops deliveries until the preference has been read", () => {
    expect(
      shouldPlayDeliveredNotificationSound({
        enabled: true,
        preferenceReady: false,
        startupSuppressed: false,
      }),
    ).toBe(false);
  });

  it("drops deliveries when the toggle is off", () => {
    expect(
      shouldPlayDeliveredNotificationSound({
        enabled: false,
        preferenceReady: true,
        startupSuppressed: false,
      }),
    ).toBe(false);
  });

  it("plays after startup when enabled and the preference is ready", () => {
    expect(
      shouldPlayDeliveredNotificationSound({
        enabled: true,
        preferenceReady: true,
        startupSuppressed: false,
      }),
    ).toBe(true);
  });
});

describe("shouldPlayCoalescedSound", () => {
  it("plays the first sound immediately", () => {
    expect(shouldPlayCoalescedSound(null, 1_000)).toBe(true);
  });

  it("ignores overlapping plays inside the coalesce window", () => {
    expect(shouldPlayCoalescedSound(1_000, 1_000 + NOTIFICATION_SOUND_COALESCE_MS - 1)).toBe(false);
  });

  it("allows another play after the coalesce window", () => {
    expect(shouldPlayCoalescedSound(1_000, 1_000 + NOTIFICATION_SOUND_COALESCE_MS)).toBe(true);
  });
});
