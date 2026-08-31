import { getNotificationSoundSettings, playNotificationSoundAlert } from "@/lib/backend";
import { onDesktopNotificationDeliver } from "@/lib/notifications";
import type { DesktopNotificationEvent } from "@/lib/types";

export const NOTIFICATION_SOUND_STORAGE_KEY = "codex-ai:notification-sound-enabled";
export const NOTIFICATION_SOUND_CHANGE_EVENT = "codex-ai:notification-sound-change";
export const NOTIFICATION_SOUND_COALESCE_MS = 400;
export const NOTIFICATION_FALLBACK_BEEP_DURATION_SECONDS = 0.2;

const TRANSIENT_DELIVERY_PREFIX = "transient:";
const FALLBACK_SAMPLE_RATE = 22050;
const FALLBACK_AMPLITUDE = 0.4;
const FALLBACK_LOW_HZ = 523.25;
const FALLBACK_HIGH_HZ = 659.25;
const STARTUP_SOUND_SUPPRESS_MS = 4000;

let enabledCache = true;
let preferenceReady = false;
let startupSoundSuppressed = true;
let startupSoundReleased = false;
let startupSoundReleaseTimer: number | null = null;
let lastCoalescedSoundAt: number | null = null;
let soundBridgeRefCount = 0;
let soundBridgeInitPromise: Promise<void> | null = null;
let soundBridgeCleanup: (() => void) | null = null;
let fallbackAudio: HTMLAudioElement | null = null;
const deliveredSoundKeys = new Set<string>();

export function parseNotificationSoundEnabled(value: string | null): boolean {
  if (value === "false") {
    return false;
  }
  return true;
}

export function buildNotificationSoundDeliveryKey(event: DesktopNotificationEvent): string {
  if (event.is_transient || event.notification_id.startsWith(TRANSIENT_DELIVERY_PREFIX)) {
    return `${TRANSIENT_DELIVERY_PREFIX}${event.notification_id}`;
  }

  return `${event.reason}:${event.notification_id}:${event.last_triggered_at}`;
}

export function shouldPlayNotificationSound(enabled: boolean, isPreview: boolean): boolean {
  return isPreview || enabled;
}

export function shouldPlayDeliveredNotificationSound(options: {
  enabled: boolean;
  preferenceReady: boolean;
  startupSuppressed: boolean;
}): boolean {
  if (options.startupSuppressed || !options.preferenceReady) {
    return false;
  }
  return shouldPlayNotificationSound(options.enabled, false);
}

export function shouldPlayCoalescedSound(
  lastPlayedAt: number | null,
  now: number,
  windowMs = NOTIFICATION_SOUND_COALESCE_MS,
): boolean {
  return lastPlayedAt == null || now - lastPlayedAt >= windowMs;
}

export function releaseStartupNotificationSounds() {
  if (startupSoundReleased) {
    return;
  }
  startupSoundReleased = true;
  startupSoundSuppressed = false;
  if (startupSoundReleaseTimer != null) {
    window.clearTimeout(startupSoundReleaseTimer);
    startupSoundReleaseTimer = null;
  }
}

function armStartupSoundSuppressionTimeout() {
  if (startupSoundReleased || startupSoundReleaseTimer != null) {
    return;
  }
  if (typeof window === "undefined") {
    return;
  }
  startupSoundReleaseTimer = window.setTimeout(() => {
    releaseStartupNotificationSounds();
  }, STARTUP_SOUND_SUPPRESS_MS);
}

export function setNotificationSoundEnabled(enabled: boolean) {
  enabledCache = enabled;
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent(NOTIFICATION_SOUND_CHANGE_EVENT, {
        detail: { enabled },
      }),
    );
  }
}

export function getNotificationSoundEnabled(): boolean {
  return enabledCache;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

export function buildNotificationBeepWavBytes(): Uint8Array {
  const durationSeconds = NOTIFICATION_FALLBACK_BEEP_DURATION_SECONDS;
  const numSamples = Math.floor(FALLBACK_SAMPLE_RATE * durationSeconds);
  const dataSize = numSamples * 2;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);

  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, FALLBACK_SAMPLE_RATE, true);
  view.setUint32(28, FALLBACK_SAMPLE_RATE * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataSize, true);

  for (let index = 0; index < numSamples; index += 1) {
    const time = index / FALLBACK_SAMPLE_RATE;
    const frequency = time < durationSeconds / 2 ? FALLBACK_LOW_HZ : FALLBACK_HIGH_HZ;
    const attack = Math.min(1, time / 0.008);
    const release = Math.min(1, (durationSeconds - time) / 0.04);
    const sample = Math.sin(2 * Math.PI * frequency * time) * attack * release * FALLBACK_AMPLITUDE;
    view.setInt16(44 + index * 2, Math.round(sample * 32767), true);
  }

  return new Uint8Array(buffer);
}

function getFallbackAudio(): HTMLAudioElement | null {
  if (typeof Audio === "undefined") {
    return null;
  }
  if (!fallbackAudio) {
    const wav = buildNotificationBeepWavBytes();
    const blob = new Blob([wav], { type: "audio/wav" });
    fallbackAudio = new Audio(URL.createObjectURL(blob));
    fallbackAudio.preload = "auto";
  }
  return fallbackAudio;
}

async function playWebFallbackSound() {
  const audio = getFallbackAudio();
  if (!audio) {
    return;
  }
  audio.muted = false;
  audio.volume = 1;
  audio.currentTime = 0;
  await audio.play();
}

export async function playNotificationSound() {
  try {
    await playNotificationSoundAlert();
    return;
  } catch {
    try {
      await playWebFallbackSound();
    } catch (error) {
      console.error("Failed to play notification sound:", error);
    }
  }
}

async function loadEnabledPreference(): Promise<boolean> {
  try {
    const settings = await getNotificationSoundSettings();
    return settings.enabled;
  } catch {
    if (typeof localStorage === "undefined") {
      return true;
    }
    return parseNotificationSoundEnabled(localStorage.getItem(NOTIFICATION_SOUND_STORAGE_KEY));
  }
}

function scheduleCoalescedNotificationSound() {
  const now = Date.now();
  if (!shouldPlayCoalescedSound(lastCoalescedSoundAt, now)) {
    return;
  }
  lastCoalescedSoundAt = now;
  void playNotificationSound();
}

function handleDeliveredNotification(event: DesktopNotificationEvent) {
  const deliveryKey = buildNotificationSoundDeliveryKey(event);
  if (deliveredSoundKeys.has(deliveryKey)) {
    return;
  }
  deliveredSoundKeys.add(deliveryKey);

  if (
    !shouldPlayDeliveredNotificationSound({
      enabled: enabledCache,
      preferenceReady,
      startupSuppressed: startupSoundSuppressed,
    })
  ) {
    return;
  }

  scheduleCoalescedNotificationSound();
}

function releaseSoundBridge() {
  soundBridgeCleanup?.();
  soundBridgeCleanup = null;
  soundBridgeInitPromise = null;
}

export function initNotificationSoundBridge() {
  soundBridgeRefCount += 1;

  if (!soundBridgeInitPromise && !soundBridgeCleanup) {
    armStartupSoundSuppressionTimeout();

    soundBridgeInitPromise = loadEnabledPreference()
      .then((enabled) => {
        enabledCache = enabled;
        preferenceReady = true;
        return onDesktopNotificationDeliver((payload) => {
          handleDeliveredNotification(payload);
        });
      })
      .then((unlisten) => {
        soundBridgeCleanup = () => {
          void unlisten();
        };
        soundBridgeInitPromise = null;

        if (soundBridgeRefCount === 0) {
          releaseSoundBridge();
        }
      })
      .catch((error) => {
        console.error("Failed to initialize notification sound bridge:", error);
        soundBridgeInitPromise = null;
        soundBridgeCleanup = null;
      });
  }

  let released = false;

  return () => {
    if (released) {
      return;
    }

    released = true;
    soundBridgeRefCount = Math.max(0, soundBridgeRefCount - 1);

    if (soundBridgeRefCount === 0 && soundBridgeCleanup) {
      releaseSoundBridge();
    }
  };
}
