import { describe, expect, it } from "vitest";

import { resolveDesktopNotificationWindowGate } from "@/lib/desktopNotifications";

describe("resolveDesktopNotificationWindowGate", () => {
  it("always sends task-linked notifications", () => {
    expect(resolveDesktopNotificationWindowGate(true, { visible: true, focused: true })).toBe(true);
    expect(resolveDesktopNotificationWindowGate(true, null)).toBe(true);
  });

  it("suppresses background-style toasts while the window is focused", () => {
    expect(resolveDesktopNotificationWindowGate(false, { visible: true, focused: true })).toBe(
      false,
    );
  });

  it("sends toasts when the window is hidden or unfocused", () => {
    expect(resolveDesktopNotificationWindowGate(false, { visible: true, focused: false })).toBe(
      true,
    );
    expect(resolveDesktopNotificationWindowGate(false, { visible: false, focused: false })).toBe(
      true,
    );
  });

  it("fails closed when window state cannot be inspected", () => {
    expect(resolveDesktopNotificationWindowGate(false, null)).toBe(false);
  });
});
