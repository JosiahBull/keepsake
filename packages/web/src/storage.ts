// IndexedDB for what Keepsake keeps on this device: saves waiting for the server, and
// copies of server data for use offline. Every call degrades to doing nothing where the
// browser refuses storage, such as some private windows.

const NAME = "keepsake";
const VERSION = 1;
export const OUTBOX = "outbox";
const COPIES = "copies";

let opened: Promise<IDBDatabase | null> | undefined;

function database(): Promise<IDBDatabase | null> {
  opened ??= new Promise((resolve) => {
    let request: IDBOpenDBRequest;
    try {
      request = indexedDB.open(NAME, VERSION);
    } catch {
      resolve(null);
      return;
    }
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(OUTBOX))
        db.createObjectStore(OUTBOX, { keyPath: "id", autoIncrement: true });
      if (!db.objectStoreNames.contains(COPIES)) db.createObjectStore(COPIES);
    };
    request.onsuccess = () => {
      const db = request.result;
      // Another tab opening a newer version must not wait on this one.
      db.onversionchange = () => db.close();
      resolve(db);
    };
    request.onerror = () => resolve(null);
    request.onblocked = () => resolve(null);
  });
  return opened;
}

function settle<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function run<T>(
  store: string,
  mode: IDBTransactionMode,
  action: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T | undefined> {
  const db = await database();
  if (!db) return undefined;
  try {
    const transaction = db.transaction(store, mode);
    // Listen before waiting on the request: the transaction can finish straight after it.
    const done = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
    const result = await settle(action(transaction.objectStore(store)));
    // A write counts only once it is committed.
    await done;
    return result;
  } catch {
    return undefined;
  }
}

/** Adds a record and returns its key, or undefined when nothing could be stored. */
export async function add<T>(store: string, value: T): Promise<number | undefined> {
  return (await run(store, "readwrite", (s) => s.add(value))) as number | undefined;
}

export async function put<T>(store: string, value: T): Promise<void> {
  await run(store, "readwrite", (s) => s.put(value));
}

export async function remove(store: string, key: IDBValidKey): Promise<void> {
  await run(store, "readwrite", (s) => s.delete(key));
}

export async function all<T>(store: string): Promise<T[]> {
  return ((await run(store, "readonly", (s) => s.getAll())) as T[] | undefined) ?? [];
}

/** A copy of server data kept for use offline. */
export async function readCopy<T>(key: string): Promise<T | undefined> {
  return (await run(COPIES, "readonly", (s) => s.get(key))) as T | undefined;
}

export async function writeCopy<T>(key: string, value: T): Promise<void> {
  await run(COPIES, "readwrite", (s) => s.put(value, key));
}

/** Removes every copy, leaving saves that have not reached the server. */
export async function clearCopies(): Promise<void> {
  await run(COPIES, "readwrite", (s) => s.clear());
}
