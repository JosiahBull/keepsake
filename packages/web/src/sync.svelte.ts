import { outbox } from "./outbox.svelte";
import { clearCopies } from "./storage";

// Offline mode keeps copies of this household's data on the device. It is on unless
// turned off in Profile settings, and applies to every profile on this device.
const offlineKey = "keepsake.offline";
/** Question images and clips kept for offline use; the service worker answers from it. */
export const MEDIA_CACHE = "keepsake-media";

class Sync {
  /** Copies being brought up to date from the server. */
  downloads = $state(0);
  /** Whether the browser says it has no network. */
  disconnected = $state(!navigator.onLine);
  offlineMode = $state(readOfflineMode());
  /** Advances when offline use is turned off, so downloads begun before it keep nothing. */
  epoch = 0;

  constructor() {
    addEventListener("online", () => (this.disconnected = false));
    addEventListener("offline", () => (this.disconnected = true));
  }

  /** True while saves or downloads are under way. */
  get syncing(): boolean {
    return outbox.sending || this.downloads > 0;
  }

  /** True when Keepsake cannot reach the server. */
  get offline(): boolean {
    return this.disconnected || outbox.offline;
  }

  /** Counts a download towards the syncing indicator until it settles. */
  async download<T>(work: () => Promise<T>): Promise<T> {
    this.downloads += 1;
    try {
      return await work();
    } finally {
      this.downloads -= 1;
    }
  }

  async setOfflineMode(on: boolean) {
    this.offlineMode = on;
    try {
      localStorage.setItem(offlineKey, on ? "on" : "off");
    } catch {
      /* Without storage, the choice lasts until Keepsake reloads. */
    }
    // Saves waiting for the server are kept either way; only copies are removed.
    if (on) return;
    this.epoch += 1;
    await clearCopies();
    if ("caches" in window) await caches.delete(MEDIA_CACHE).catch(() => false);
  }
}

function readOfflineMode(): boolean {
  try {
    return localStorage.getItem(offlineKey) !== "off";
  } catch {
    return true;
  }
}

export const sync = new Sync();
