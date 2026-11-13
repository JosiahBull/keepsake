import type { components } from "@keepsake/client";
import { json } from "./profiles";
import { clearCopies, readCopy, writeCopy } from "./storage";
import { MEDIA_CACHE, sync } from "./sync.svelte";

type Fact = components["schemas"]["Fact"];
type CopyPage = components["schemas"]["CopyPage"];
type FactPage = components["schemas"]["FactPage"];
type PracticeSession = components["schemas"]["PracticeSession"];
type FactSample = components["schemas"]["FactSample"];
type Media = components["schemas"]["QuestionMedia"];

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
  const epoch = sync.epoch;
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
      if (epoch !== sync.epoch) return;
      await writeCopy<LibraryCopy>(key(profileId), { facts, savedAt: Date.now() });
      if (epoch !== sync.epoch) {
        await clearCopies();
        return;
      }
      refreshed.set(profileId, Date.now());
      void keepMedia(profileId, facts, epoch);
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
async function keepMedia(profileId: number, facts: Fact[], epoch: number) {
  if (!("caches" in window)) return;
  const wanted = new Set(
    facts.flatMap((fact) => fact.media.map((item) => mediaPath(profileId, item.media_id))),
  );
  if (!wanted.size && !(await caches.has(MEDIA_CACHE))) return;
  void navigator.storage?.persist?.().catch(() => false);
  // Turned off part way: what this download kept goes too.
  const stale = async () => {
    if (epoch === sync.epoch) return false;
    await caches.delete(MEDIA_CACHE);
    return true;
  };
  await sync.download(async () => {
    const cache = await caches.open(MEDIA_CACHE);
    const prefix = mediaPath(profileId, 0).slice(0, -1);
    for (const request of await cache.keys()) {
      const path = new URL(request.url).pathname;
      if (path.startsWith(prefix) && !wanted.has(path)) await cache.delete(request);
    }
    for (const path of wanted) {
      if (await stale()) return;
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
    await stale();
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

/**
 * Whether every image and clip a question shows is kept on this device. A question asked
 * without its picture or sound is a different question, so offline practice skips it.
 */
async function complete(profileId: number, facts: Fact[]): Promise<Fact[]> {
  if (!facts.some((fact) => fact.media.length)) return facts;
  const cache = "caches" in window ? await caches.open(MEDIA_CACHE) : null;
  const kept = async (item: Media) =>
    Boolean(cache && (await cache.match(mediaPath(profileId, item.media_id))));
  const usable: Fact[] = [];
  for (const fact of facts)
    if ((await Promise.all(fact.media.map(kept))).every(Boolean)) usable.push(fact);
  return usable;
}

function shuffled<T>(items: T[]): T[] {
  const result = [...items];
  for (let index = result.length - 1; index > 0; index--) {
    const other = Math.floor(Math.random() * (index + 1));
    [result[index], result[other]] = [result[other], result[index]];
  }
  return result;
}

/** Extra practice drawn at random from every prompt in this device's copy. */
export async function practiceFromCopy(
  profileId: number,
  limit = 20,
): Promise<PracticeSession | undefined> {
  const copy = await readLibrary(profileId);
  if (!copy) return undefined;
  const questions = (await complete(profileId, copy.facts)).flatMap(
    ({ practices, ...fact }) => practices.map((practice) => ({ fact, practice })),
  );
  return { items: shuffled(questions).slice(0, limit), total: questions.length };
}

/**
 * A matching round from this device's copy, chosen as the server chooses one: facts
 * sharing a topic tag, with no question that has two answers, from the smallest bank
 * holding a randomly chosen pair. Empty when no tag has three distinct pairs.
 */
export async function matchingFromCopy(
  profileId: number,
  limit = 4,
): Promise<FactSample | undefined> {
  const copy = await readLibrary(profileId);
  if (!copy) return undefined;
  const facts = await complete(profileId, copy.facts);
  const caseless = (value: string) => value.trim().toLocaleLowerCase();
  const question = (fact: Fact) =>
    [caseless(fact.prompt), ...fact.media.map((item) => item.media_id)].join("\u001f");
  const tags = (fact: Fact) => new Set(fact.tags.map(caseless).filter(Boolean));
  const banks = new Map<string, Fact[]>();
  for (const fact of facts)
    for (const tag of tags(fact)) banks.set(tag, [...(banks.get(tag) ?? []), fact]);
  const pairs = new Map<string, Fact[]>();
  for (const [tag, members] of banks) {
    // A question with two different answers in one bank cannot be matched fairly.
    const answers = new Map<string, Set<string>>();
    for (const fact of members)
      answers.set(question(fact), (answers.get(question(fact)) ?? new Set()).add(caseless(fact.answer)));
    const byAnswer = new Map<string, Fact>();
    for (const fact of [...members].sort((left, right) => left.id - right.id))
      if (answers.get(question(fact))!.size === 1 && !byAnswer.has(caseless(fact.answer)))
        byAnswer.set(caseless(fact.answer), fact);
    if (byAnswer.size >= 3) pairs.set(tag, [...byAnswer.values()]);
  }
  const seeds = [...new Set([...pairs.values()].flat())];
  if (!seeds.length) return { items: [] };
  const seed = seeds[Math.floor(Math.random() * seeds.length)];
  const [bank] = shuffled([...pairs.entries()].filter(([, members]) => members.includes(seed)))
    .sort(([, left], [, right]) => left.length - right.length)[0];
  const seedTags = tags(seed);
  const shared = (fact: Fact) => [...tags(fact)].filter((tag) => seedTags.has(tag)).length;
  const others = shuffled(pairs.get(bank)!.filter((fact) => fact !== seed)).sort(
    (left, right) => shared(right) - shared(left),
  );
  const items = [seed, ...others.slice(0, limit - 1)].map(({ practices: _, ...fact }) => fact);
  return { items };
}
