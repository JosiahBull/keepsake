import type { components } from "@keepsake/client";
import { json, RequestError } from "./profiles";

export type ReminderSettings = components["schemas"]["ReminderSettings"];
type StudySummary = components["schemas"]["StudySummary"];
type PushKey = components["schemas"]["PushKey"];

// Profiles that turned reminders on in this browser. The server holds the subscription.
const enabledKey = "keepsake.reminders";

let registration: Promise<ServiceWorkerRegistration | null> | undefined;

/** Register the service worker once; null where this browser cannot run it. */
export function serviceWorker(): Promise<ServiceWorkerRegistration | null> {
  registration ??=
    "serviceWorker" in navigator && window.isSecureContext
      ? navigator.serviceWorker.register("/sw.js").catch(() => null)
      : Promise.resolve(null);
  return registration;
}

export function standalone(): boolean {
  return (
    matchMedia("(display-mode: standalone)").matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true
  );
}

function appleTouchDevice(): boolean {
  // iPadOS reports a Mac user agent; touch support distinguishes it.
  return (
    /iPhone|iPad|iPod/.test(navigator.userAgent) ||
    (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1)
  );
}

export type ReminderSupport =
  | "available"
  | "install"
  | "insecure"
  | "unsupported"
  | "blocked";

export function reminderSupport(): ReminderSupport {
  if (!window.isSecureContext) return "insecure";
  if (
    !("serviceWorker" in navigator) ||
    !("PushManager" in window) ||
    !("Notification" in window)
  ) {
    // iOS and iPadOS offer Web Push only to Home Screen apps.
    return appleTouchDevice() && !standalone() ? "install" : "unsupported";
  }
  return Notification.permission === "denied" ? "blocked" : "available";
}

function enabledProfiles(): number[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(enabledKey) ?? "[]");
    return Array.isArray(value) ? value.filter(Number.isInteger) : [];
  } catch {
    return [];
  }
}

function rememberProfiles(ids: number[]) {
  try {
    localStorage.setItem(enabledKey, JSON.stringify(ids));
  } catch {
    /* Without storage, reminders stay on until turned off. */
  }
}

function decode(base64url: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64url.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function sameBytes(left: ArrayBuffer | null, right: Uint8Array): boolean {
  if (!left || left.byteLength !== right.length) return false;
  const bytes = new Uint8Array(left);
  return bytes.every((byte, index) => byte === right[index]);
}

function device(subscription: PushSubscription) {
  const { endpoint, keys } = subscription.toJSON();
  return { endpoint, keys: { p256dh: keys?.p256dh, auth: keys?.auth } };
}

async function subscription(): Promise<PushSubscription | null> {
  if (reminderSupport() !== "available") return null;
  const worker = await serviceWorker();
  return worker ? worker.pushManager.getSubscription() : null;
}

/**
 * Whether this browser receives the profile's reminders. Re-registering repairs a
 * subscription the server forgot or the browser rotated.
 */
export async function remindersOn(profileId: number): Promise<boolean> {
  if (
    !enabledProfiles().includes(profileId) ||
    reminderSupport() !== "available" ||
    Notification.permission !== "granted"
  )
    return false;
  const current = await subscription();
  if (!current) {
    rememberProfiles(enabledProfiles().filter((id) => id !== profileId));
    return false;
  }
  await json(`/api/profiles/${profileId}/reminders/devices`, {
    method: "PUT",
    body: JSON.stringify(device(current)),
  });
  return true;
}

export async function enableReminders(
  profileId: number,
): Promise<ReminderSettings> {
  // Safari shows the prompt only while handling the tap, so ask before any await.
  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error(
      permission === "denied"
        ? "Notifications are blocked for Keepsake."
        : "Notifications were not allowed.",
    );
  }
  if (!(await serviceWorker()))
    throw new Error("This browser cannot receive reminders.");
  const worker = await navigator.serviceWorker.ready;
  const key = decode((await json<PushKey>("/api/push/key")).public_key);
  let current = await worker.pushManager.getSubscription();
  if (current && !sameBytes(current.options.applicationServerKey, key)) {
    // A subscription for another server key cannot receive this server's messages.
    await current.unsubscribe();
    current = null;
  }
  current ??= await worker.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: key,
  });
  const settings = await json<ReminderSettings>(
    `/api/profiles/${profileId}/reminders/devices`,
    { method: "PUT", body: JSON.stringify(device(current)) },
  );
  rememberProfiles([
    ...enabledProfiles().filter((id) => id !== profileId),
    profileId,
  ]);
  return settings;
}

export async function disableReminders(
  profileId: number,
): Promise<ReminderSettings> {
  const current = await subscription();
  const settings = current
    ? await json<ReminderSettings>(
        `/api/profiles/${profileId}/reminders/devices`,
        {
          method: "DELETE",
          body: JSON.stringify({ endpoint: current.endpoint }),
        },
      )
    : await json<ReminderSettings>(`/api/profiles/${profileId}/reminders`);
  const remaining = enabledProfiles().filter((id) => id !== profileId);
  rememberProfiles(remaining);
  // Other household profiles on this browser keep the shared subscription.
  if (current && remaining.length === 0) await current.unsubscribe();
  return settings;
}

export async function testReminder(profileId: number): Promise<void> {
  const current = await subscription();
  if (!current) throw new Error("Reminders are not on for this device.");
  const response = await fetch(`/api/profiles/${profileId}/reminders/test`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ endpoint: current.endpoint }),
  });
  if (!response.ok) {
    const body: unknown = await response.json().catch(() => null);
    throw new RequestError(
      typeof body === "object" &&
      body !== null &&
      "error" in body &&
      typeof body.error === "string"
        ? body.error
        : `Request failed (${response.status}).`,
      response.status,
    );
  }
}

/** Show ready practice on the Home Screen icon of an installed app. */
export async function refreshBadge(profileId: number): Promise<void> {
  if (
    !standalone() ||
    !("setAppBadge" in navigator) ||
    !("Notification" in window) ||
    Notification.permission !== "granted"
  )
    return;
  try {
    const summary = await json<StudySummary>(
      `/api/profiles/${profileId}/study/summary`,
    );
    const count = summary.due + summary.new_available;
    if (count > 0) await navigator.setAppBadge(count);
    else await navigator.clearAppBadge();
  } catch {
    /* Badges are optional. */
  }
}
