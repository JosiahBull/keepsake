import type { components } from "@keepsake/client";
import { json } from "./profiles";
import { readCopy, writeCopy } from "./storage";
import { MEDIA_CACHE, sync } from "./sync.svelte";

type Fact = components["schemas"]["Fact"];
type CopyPage = components["schemas"]["CopyPage"];
type FactPage = components["schemas"]["FactPage"];

/** A profile's whole library as this device last downloaded it, newest fact first. */
export type LibraryCopy = { facts: Fact[]; savedAt: number };

// Downloading again within this time adds little: the copy is for when the server is gone.
const FRESH_MS = 60_000;
const key = (profileId: number) => `library:${profileId}`;
const refreshing = new Map<number, Promise<void>>();
const refreshed = new Map<number, number>();
let latest: number | null = null;

addEventListener("online", () => {
  if (latest !== null) void refreshLibrary(latest, true);
});

export async function readLibrary(profileId: number): Promise<LibraryCopy | undefined> {
  return sync.offlineMode ? readCopy<LibraryCopy>(key(profileId)) : undefined;
}

/**
 * Downloads the profile's library, then as many of its images and clips as fit, so it
 * can be used offline. Does nothing with offline use turned off, or if it ran recently.
 */
export function refreshLibrary(profileId: number, force = false): Promise<void> {
  latest = profileId;
  if (!sync.offlineMode) return Promise.resolve();
  const running = refreshing.get(profileId);
  if (running) return running;
  if (!force && Date.now() - (refreshed.get(profileId) ?? 0) < FRESH_MS)
    return Promise.resolve();
  const work = sync
    .download(async () => {
      const facts: Fact[] = [];
      let before: number | null | undefined;
      do {
        const query = before ? `&before_id=${before}` : "";
        const page = await json<CopyPage>(
          `/api/profiles/${profileId}/facts/copy?limit=100${query}`,
        );
        facts.push(...page.items);
        before = page.next_cursor;
      } while (before);
      // Turned off while downloading: keep nothing.
      if (!sync.offlineMode) return;
      await writeCopy<LibraryCopy>(key(profileId), { facts, savedAt: Date.now() });
      refreshed.set(profileId, Date.now());
      void keepMedia(profileId, facts);
    })
    // Offline, or the server failed: the copy already kept stays as it is.
    .catch(() => undefined)
    .finally(() => refreshing.delete(profileId));
  refreshing.set(profileId, work);
  return work;
}

const mediaPath = (profileId: number, mediaId: number) =>
  `/api/profiles/${profileId}/media/${mediaId}`;

/**
 * Keeps the library's images and clips in the cache the service worker answers from,
 * newest fact first, stopping before the device runs short of space.
 */
async function keepMedia(profileId: number, facts: Fact[]) {
  if (!("caches" in window)) return;
  void navigator.storage?.persist?.().catch(() => false);
  await sync.download(async () => {
    const cache = await caches.open(MEDIA_CACHE);
    const wanted = new Set(
      facts.flatMap((fact) => fact.media.map((item) => mediaPath(profileId, item.media_id))),
    );
    const prefix = mediaPath(profileId, 0).slice(0, -1);
    for (const request of await cache.keys()) {
      const path = new URL(request.url).pathname;
      if (path.startsWith(prefix) && !wanted.has(path)) await cache.delete(request);
    }
    for (const path of wanted) {
      if (!sync.offlineMode) return;
      if (await cache.match(path)) continue;
      if (!(await room())) return;
      try {
        const response = await fetch(path);
        if (response.ok) await cache.put(path, response);
      } catch {
        // Offline again, or out of space: what is kept stays.
        return;
      }
    }
  }).catch(() => undefined);
}

/** Leaves a tenth of the browser's quota, and at least 50 MB, for everything else. */
async function room(): Promise<boolean> {
  const estimate = await navigator.storage?.estimate?.().catch(() => undefined);
  if (!estimate?.quota) return true;
  const margin = Math.max(50_000_000, estimate.quota / 10);
  return (estimate.usage ?? 0) < estimate.quota - margin;
}

/** Folds case as the server's search does: ASCII letters and Māori macron vowels. */
function fold(value: string): string {
  return value
    .replace(/[A-Z]/g, (letter) => letter.toLowerCase())
    .replace(/[ĀĒĪŌŪ]/g, (letter) => letter.toLowerCase());
}

/** A page of the library from this device's copy, searched and ordered as the server does. */
export function pageFromCopy(
  copy: LibraryCopy,
  { limit, before_id, search }: { limit: number; before_id?: number; search: string },
): FactPage {
  const needle = fold(search);
  // The server searches tags as their stored JSON text.
  const matches = copy.facts.filter((fact) =>
    [fact.prompt, fact.answer, fact.context, JSON.stringify(fact.tags)].some((field) =>
      fold(field).includes(needle),
    ),
  );
  const after = matches.filter((fact) => before_id === undefined || fact.id < before_id);
  const items = after.slice(0, limit);
  return {
    items,
    total: matches.length,
    next_cursor: after.length > limit ? items[items.length - 1].id : null,
  };
}
