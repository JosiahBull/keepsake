/** A save the server refused. Sending the same request again cannot succeed. */
export class Rejected extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

type Save = {
  label: string;
  send: () => Promise<unknown>;
  resolve: (value: unknown) => void;
  reject: (error: unknown) => void;
};

/**
 * Sends saves one at a time, in the order they were made, so the server sees the same
 * sequence as an interface that waited for each one. A failed save holds back later
 * saves until it is retried with the same request. A refused save is dropped.
 */
export class SaveQueue {
  /** Saves not yet confirmed, including one waiting for a retry. */
  size = $state(0);
  failure = $state<{ label: string; error: unknown } | null>(null);
  #saves: Save[] = [];
  #idle: (() => void)[] = [];
  #running = false;

  /** Resolves when the server confirms the save and rejects only if it refuses it. */
  add<T>(label: string, send: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.#saves.push({ label, send, resolve: resolve as (value: unknown) => void, reject });
      this.size = this.#saves.length;
      void this.#run();
    });
  }

  retry() {
    this.failure = null;
    void this.#run();
  }

  /** Resolves once every save made so far has been confirmed or refused. */
  idle(): Promise<void> {
    return this.#saves.length
      ? new Promise((resolve) => this.#idle.push(resolve))
      : Promise.resolve();
  }

  async #run() {
    if (this.#running || this.failure) return;
    this.#running = true;
    while (this.#saves.length) {
      const save = this.#saves[0];
      try {
        save.resolve(await save.send());
      } catch (error) {
        if (!(error instanceof Rejected)) {
          this.failure = { label: save.label, error };
          break;
        }
        save.reject(error);
      }
      this.#saves.shift();
      this.size = this.#saves.length;
    }
    this.#running = false;
    if (!this.#saves.length) for (const resolve of this.#idle.splice(0)) resolve();
  }
}
