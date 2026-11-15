import { fetchPreloaded } from "./preload";
import { add, all, OUTBOX, put, remove } from "./storage";

/** A save the server refused. Sending the same request again cannot succeed. */
export class Rejected extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

/** The request never reached the server, as when the device is offline. */
export class Unreachable extends Error {}

/** A save as it is kept on this device: everything needed to send it again. */
type Stored = {
  id?: number;
  /** The tab sending it. Another tab takes it over once that tab has closed. */
  owner: string;
  label: string;
  method: string;
  path: string;
  body: string | null;
  headers?: Record<string, string>;
  /** Files for a new fact, uploaded before it; each keeps its media ID once uploaded. */
  uploads?: Upload[];
};

/**
 * An image or clip for a fact added offline, kept until it is uploaded. Its bytes are an
 * ArrayBuffer because private windows refuse to keep Blobs in IndexedDB.
 */
export type Upload = { data: ArrayBuffer; type: string; alt: string; mediaId?: number };

type Save = Stored & {
  /** Settles once the save is kept on this device, or could not be. */
  kept?: Promise<void>;
  /** Counted once kept, so the count shown is safe to close Keepsake on. */
  counted?: boolean;
  /** For a save the learner may stop waiting for once it is kept and the server is unreachable. */
  release?: () => void;
  resolve?: (value: unknown) => void;
  reject?: (error: unknown) => void;
};

export type SaveRequest = {
  method: string;
  path: string;
  body?: unknown;
  headers?: Record<string, string>;
  uploads?: Upload[];
};

const tab = crypto.randomUUID();
const tabLock = (owner: string) => `keepsake-tab:${owner}`;

/**
 * Sends saves one at a time, in the order they were made, so the server sees the same
 * sequence as an interface that waited for each one. Each save is kept on this device
 * until the server confirms or refuses it, so closing Keepsake or losing the connection
 * loses nothing: a later visit sends it. A failed save holds back later saves until it
 * is retried with the same request. A refused save is dropped.
 */
class Outbox {
  /** Saves kept on this device and not yet confirmed, including one waiting for a retry. */
  size = $state(0);
  failure = $state<{ label: string; error: unknown } | null>(null);
  /** True while a save is on its way to the server. */
  sending = $state(false);
  /** Saves from an earlier visit that the server refused, to report once. */
  refused = $state<string[]>([]);
  /** Attempts made offline that the server kept without applying, to report once. */
  ignored = $state(0);
  /** Edits made offline that the server refused, to offer again in the edit form. */
  refusedEdits = $state<{ path: string; body: string; status: number; reason: string }[]>([]);
  /** Facts added offline that have not reached the server, by profile path and question. */
  queuedFacts = $state<{ path: string; prompt: string }[]>([]);
  #saves: Save[] = [];
  #idle: (() => void)[] = [];
  #settled: ((sent: boolean) => void)[] = [];
  #watched = new Set<string>();
  #running = false;
  #ready: Promise<void>;

  constructor() {
    this.#ready = this.#claimTab().then(() => this.#adopt());
    // Saves left by an earlier visit go as soon as this one starts.
    void this.#ready.then(() => this.#run());
    addEventListener("online", () => this.resume());
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") this.resume();
    });
  }

  /**
   * Keeps a save and sends it in turn. The promise resolves when the server confirms it,
   * and rejects if the server refuses it. With `until: "stored"` it also resolves once the
   * save is kept on this device while the server cannot be reached, so offline use goes on.
   */
  add<T>(
    label: string,
    request: SaveRequest,
    { until = "sent" }: { until?: "sent" | "stored" } = {},
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const save: Save = {
        owner: tab,
        label,
        method: request.method,
        path: request.path,
        body: request.body === undefined ? null : JSON.stringify(request.body),
        headers: request.headers,
        uploads: request.uploads,
      };
      const kept = this.#ready
        .then(() => add<Stored>(OUTBOX, stored(save)))
        .then((id) => {
          save.id = id;
          save.counted = true;
          this.#count();
        });
      save.kept = kept;
      if (until === "stored") {
        let waiting = true;
        const stop = () => {
          if (!waiting) return false;
          waiting = false;
          return true;
        };
        save.resolve = (value) => stop() && resolve(value as T);
        // Once released, a refusal can only be reported, as for an earlier visit's save.
        save.reject = (error) => (stop() ? reject(error) : this.#refuse(save, error));
        save.release = () => void kept.then(() => stop() && resolve(undefined as T));
        if (this.offline) save.release();
      } else {
        save.resolve = resolve as (value: unknown) => void;
        save.reject = reject;
      }
      this.#saves.push(save);
      void kept.then(() => this.#run());
    });
  }

  retry() {
    this.failure = null;
    void this.#run();
  }

  /** Tries again after the connection returns, taking over saves left by closed tabs. */
  resume() {
    void this.#adopt().then(() => {
      if (this.failure) this.retry();
      else void this.#run();
    });
  }

  /** Resolves true once every save so far is confirmed or refused, or false if one fails. */
  settled(): Promise<boolean> {
    if (this.failure) return Promise.resolve(false);
    if (!this.#saves.length) return Promise.resolve(true);
    return new Promise((resolve) => this.#settled.push(resolve));
  }

  /** Resolves once every save made so far has been confirmed or refused. */
  idle(): Promise<void> {
    return this.#saves.length
      ? new Promise((resolve) => this.#idle.push(resolve))
      : Promise.resolve();
  }

  /** Whether the latest failure was the connection rather than the server. */
  get offline(): boolean {
    return this.failure?.error instanceof Unreachable;
  }

  async #run() {
    await this.#ready;
    if (this.#running || this.failure) return;
    this.#running = true;
    this.sending = true;
    while (this.#saves.length) {
      const save = this.#saves[0];
      // A save is sent only once it is kept, so a confirmed save is never sent again.
      await save.kept;
      try {
        // Sent whether or not anything here waits for the answer, as for an earlier visit's.
        const result = await send(save);
        if (ignoredAttempt(save, result)) this.ignored += 1;
        save.resolve?.(result);
      } catch (error) {
        if (!(error instanceof Rejected)) {
          this.failure = { label: save.label, error };
          for (const resolve of this.#settled.splice(0)) resolve(false);
          if (error instanceof Unreachable) for (const waiting of this.#saves) waiting.release?.();
          break;
        }
        if (save.reject) save.reject(error);
        else this.#refuse(save, error);
      }
      this.#saves.shift();
      this.#count();
      if (save.id !== undefined) await remove(OUTBOX, save.id);
    }
    this.#running = false;
    this.sending = false;
    if (!this.#saves.length) {
      for (const resolve of this.#idle.splice(0)) resolve();
      for (const resolve of this.#settled.splice(0)) resolve(true);
    }
  }

  /** Takes over another tab's saves once it closes, which releases its lock. */
  #watch(owner: string) {
    if (this.#watched.has(owner)) return;
    this.#watched.add(owner);
    void navigator.locks
      .request(tabLock(owner), async () => undefined)
      .then(() => this.resume());
  }

  #count() {
    this.size = this.#saves.filter((save) => save.counted).length;
    this.queuedFacts = this.#saves.filter(addsFact).map((save) => ({
      path: save.path,
      prompt: (JSON.parse(save.body ?? "{}") as { prompt?: string }).prompt ?? "",
    }));
  }

  #refuse(save: Save, error: unknown) {
    const reason = error instanceof Error ? error.message : "";
    if (save.method === "PATCH" && error instanceof Rejected && save.body !== null) {
      this.refusedEdits = [
        ...this.refusedEdits,
        { path: save.path, body: save.body, status: error.status, reason },
      ];
      return;
    }
    this.refused = [...this.refused, `${save.label} ${reason}`.trim()];
  }

  /** Holds a lock for as long as this tab is open, so other tabs leave its saves alone. */
  async #claimTab() {
    if (!navigator.locks) return;
    await new Promise<void>((granted) => {
      void navigator.locks.request(tabLock(tab), () => {
        granted();
        return new Promise(() => undefined);
      });
    });
  }

  /** Takes over saves kept by tabs that have since closed, ahead of this tab's own. */
  async #adopt() {
    const take = async () => {
      const held = new Set(
        navigator.locks
          ? ((await navigator.locks.query()).held ?? []).map((lock) => lock.name)
          : [tabLock(tab)],
      );
      const known = new Set(this.#saves.map((save) => save.id));
      const kept = await all<Stored>(OUTBOX);
      for (const save of kept)
        if (save.owner !== tab && held.has(tabLock(save.owner))) this.#watch(save.owner);
      const orphans = kept
        .filter((save) => !held.has(tabLock(save.owner)) && !known.has(save.id))
        .sort((left, right) => left.id! - right.id!);
      for (const save of orphans) {
        save.owner = tab;
        await put(OUTBOX, save);
      }
      return orphans;
    };
    const orphans = navigator.locks
      ? await navigator.locks.request("keepsake-outbox-adopt", take)
      : await take();
    if (!orphans.length) return;
    // A save being sent stays first; the orphans are older than everything after it.
    const start = this.#running ? 1 : 0;
    this.#saves.splice(start, 0, ...orphans.map((save) => ({ ...save, counted: true })));
    this.#count();
  }
}

/** A rating made offline that arrived after its target changed, so it did not count. */
function ignoredAttempt(save: Stored, result: unknown): boolean {
  if (typeof result !== "object" || result === null || !("ignored" in result)) return false;
  try {
    return Boolean(result.ignored) && JSON.parse(save.body ?? "{}").kind === "attempt";
  } catch {
    return false;
  }
}

function stored({ owner, label, method, path, body, headers, uploads }: Save): Stored {
  return { owner, label, method, path, body, headers, uploads };
}

const addsFact = (save: Stored) => save.method === "POST" && /\/facts$/.test(save.path);

async function send(save: Stored): Promise<unknown> {
  let body = save.body;
  if (save.uploads?.length) {
    const media = save.path.replace(/\/facts$/, "/media");
    for (const upload of save.uploads) {
      if (upload.mediaId !== undefined) continue;
      const uploaded = (await request(media, "POST", new Blob([upload.data]), {
        "Content-Type": upload.type,
      })) as { media_id: number };
      upload.mediaId = uploaded.media_id;
      // A retry after this point reuses the upload instead of sending the file again.
      if (save.id !== undefined) await put(OUTBOX, { ...stored(save), id: save.id });
    }
    body = JSON.stringify({
      ...JSON.parse(body ?? "{}"),
      media: save.uploads.map((upload) => ({ media_id: upload.mediaId, alt: upload.alt })),
    });
  }
  // A fact deleted already, perhaps on another device, is deleted either way.
  if (save.method === "DELETE")
    return request(save.path, save.method, body, save.headers ?? {}).catch((error) => {
      if (error instanceof Rejected && error.status === 404) return undefined;
      throw error;
    });
  return request(save.path, save.method, body, {
    ...(body === null ? {} : { "Content-Type": "application/json" }),
    ...save.headers,
  });
}

async function request(
  path: string,
  method: string,
  body: BodyInit | null,
  headers: Record<string, string>,
): Promise<unknown> {
  let response: Response;
  try {
    response = await fetchPreloaded(new Request(path, { method, headers, body }));
  } catch {
    throw new Unreachable("You are offline. Keepsake will send it when you reconnect.");
  }
  const answer: unknown = await response.json().catch(() => undefined);
  if (response.ok) return answer;
  const message =
    typeof answer === "object" &&
    answer !== null &&
    "error" in answer &&
    typeof answer.error === "string"
      ? answer.error
      : `Request failed (${response.status}).`;
  const { status } = response;
  if (status >= 400 && status < 500 && status !== 408 && status !== 429)
    throw new Rejected(message, status);
  throw new Error(message);
}

export const outbox = new Outbox();
