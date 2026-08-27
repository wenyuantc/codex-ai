import { getNotificationSoundSettings, playNotificationSoundAlert } from "@/lib/backend";
import { onDesktopNotificationDeliver } from "@/lib/notifications";
import type { DesktopNotificationEvent } from "@/lib/types";

export const NOTIFICATION_SOUND_STORAGE_KEY = "codex-ai:notification-sound-enabled";
export const NOTIFICATION_SOUND_CHANGE_EVENT = "codex-ai:notification-sound-change";

const TRANSIENT_DELIVERY_PREFIX = "transient:";
const FALLBACK_SAMPLE_RATE = 22050;
const FALLBACK_DURATION_SECONDS = 1.4;
const FALLBACK_AMPLITUDE = 0.88;

let enabledCache = true;
let soundBridgeRefCount = 0;
let soundBridgeInitPromise: Promise<void> | null = null;
let soundBridgeCleanup: (() => void) | null = null;
let fallbackAudio: HTMLAudioElement | null = null;
let fallbackUnlocked = false;
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
  const numSamples = Math.floor(FALLBACK_SAMPLE_RATE * FALLBACK_DURATION_SECONDS);
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
    const frequency = time < 0.45 ? 784 : 1046.5;
    const attack = Math.min(1, time / 0.02);
    const release = Math.min(1, (FALLBACK_DURATION_SECONDS - time) / 0.12);
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

async function unlockWebFallbackAudio() {
  const audio = getFallbackAudio();
  if (!audio || fallbackUnlocked) {
    return;
  }

  audio.muted = true;
  audio.volume = 0;
  try {
    await audio.play();
    audio.pause();
    audio.currentTime = 0;
    fallbackUnlocked = true;
  } catch {
    // Gesture unlock is best-effort; Tauri plays a system sound instead.
  } finally {
    audio.muted = false;
    audio.volume = 1;
  }
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

function handleDeliveredNotification(event: DesktopNotificationEvent) {
  const deliveryKey = buildNotificationSoundDeliveryKey(event);
  if (deliveredSoundKeys.has(deliveryKey)) {
    return;
  }
  deliveredSoundKeys.add(deliveryKey);

  if (!shouldPlayNotificationSound(enabledCache, false)) {
    return;
  }

  void playNotificationSound();
}

function releaseSoundBridge() {
  soundBridgeCleanup?.();
  soundBridgeCleanup = null;
  soundBridgeInitPromise = null;
}

export function initNotificationSoundBridge() {
  soundBridgeRefCount += 1;

  if (!soundBridgeInitPromise && !soundBridgeCleanup) {
    const handleUnlock = () => {
      void unlockWebFallbackAudio();
    };
    window.addEventListener("pointerdown", handleUnlock, true);
    window.addEventListener("keydown", handleUnlock, true);
    void loadEnabledPreference().then((enabled) => {
      enabledCache = enabled;
    });

    soundBridgeInitPromise = onDesktopNotificationDeliver((payload) => {
      handleDeliveredNotification(payload);
    })
      .then((unlisten) => {
        soundBridgeCleanup = () => {
          window.removeEventListener("pointerdown", handleUnlock, true);
          window.removeEventListener("keydown", handleUnlock, true);
          void unlisten();
        };
        soundBridgeInitPromise = null;

        if (soundBridgeRefCount === 0) {
          releaseSoundBridge();
        }
      })
      .catch((error) => {
        console.error("Failed to initialize notification sound bridge:", error);
        window.removeEventListener("pointerdown", handleUnlock, true);
        window.removeEventListener("keydown", handleUnlock, true);
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
