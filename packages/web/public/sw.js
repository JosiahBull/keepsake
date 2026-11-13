// Reminders, app badges, an offline app shell and question media kept for offline use.
// Other API responses are never cached here; the app keeps its own copies.
const SHELL = "keepsake-shell-v2";
// The app fills this with question images and clips for offline use. Keep the name in
// step with MEDIA_CACHE in src/sync.svelte.ts.
const MEDIA = "keepsake-media";
const MEDIA_PATH = /^\/api\/profiles\/\d+\/media\/\d+$/;
const PRECACHE = [
  "/keepsake-logo.svg",
  "/keepsake-icon.png",
  "/manifest.webmanifest",
  "/icons/apple-touch-icon.png",
  "/icons/icon-192.png",
  "/icons/badge-96.png",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      try {
        const cache = await caches.open(SHELL);
        await cache.addAll(PRECACHE);
        await storeShell(await fetch("/"));
      } catch {
        // An offline install still handles notifications; the shell is cached on the next launch.
      }
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) {
        if (key !== SHELL && key !== MEDIA) await caches.delete(key);
      }
      await self.clients.claim();
    })(),
  );
});

// Keep the newest shell with the fingerprinted assets it references, and no others.
async function storeShell(response) {
  if (!response.ok) return;
  const cache = await caches.open(SHELL);
  const html = await response.clone().text();
  const assets = new Set(html.match(/\/assets\/[^"'\s>]+/g) ?? []);
  await cache.addAll([...assets]);
  await cache.put("/", response);
  for (const request of await cache.keys()) {
    const path = new URL(request.url).pathname;
    if (path.startsWith("/assets/") && !assets.has(path)) await cache.delete(request);
  }
}

async function navigate(event) {
  const { request } = event;
  try {
    const response = await fetch(request);
    // Every page is the same shell, so each launch refreshes the offline copy. Caching
    // must not delay or fail the launch it is preparing for.
    if (response.headers.get("content-type")?.startsWith("text/html"))
      event.waitUntil(storeShell(response.clone()).catch(() => undefined));
    return response;
  } catch (error) {
    // Every route renders the same single-page shell, which reports the connection itself.
    const cached = await caches.match("/");
    if (cached) return cached;
    throw error;
  }
}

// Fingerprinted assets never change, so a cached copy is always current.
async function asset(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(SHELL);
    await cache.put(request, response.clone());
  }
  return response;
}

// A media file never changes once stored, so a kept copy answers first, byte ranges included.
async function media(request) {
  const cached = await (await caches.open(MEDIA)).match(request.url);
  if (!cached) return fetch(request);
  const range = /^bytes=(\d*)-(\d*)$/.exec(request.headers.get("range") ?? "");
  if (!range) return cached;
  const body = await cached.blob();
  const size = body.size;
  let start = range[1] ? Number(range[1]) : size - Number(range[2]);
  let end = range[1] && range[2] ? Number(range[2]) : size - 1;
  start = Math.max(0, start);
  end = Math.min(end, size - 1);
  if (start > end) {
    return new Response(null, { status: 416, headers: { "Content-Range": `bytes */${size}` } });
  }
  return new Response(body.slice(start, end + 1), {
    status: 206,
    headers: {
      "Content-Type": cached.headers.get("content-type") ?? "application/octet-stream",
      "Content-Range": `bytes ${start}-${end}/${size}`,
      "Content-Length": String(end - start + 1),
      "Accept-Ranges": "bytes",
    },
  });
}

async function precached(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(SHELL);
      await cache.put(request, response.clone());
    }
    return response;
  } catch (error) {
    const cached = await caches.match(request);
    if (cached) return cached;
    throw error;
  }
}

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin) return;
  if (MEDIA_PATH.test(url.pathname)) {
    event.respondWith(media(request));
    return;
  }
  if (url.pathname === "/api" || url.pathname.startsWith("/api/")) return;
  if (request.mode === "navigate") event.respondWith(navigate(event));
  else if (url.pathname.startsWith("/assets/")) event.respondWith(asset(request));
  else if (PRECACHE.includes(url.pathname)) {
    event.respondWith(precached(request));
  }
});

async function setBadge(value) {
  if (!("setAppBadge" in self.navigator)) return;
  const count = Number(value);
  try {
    if (count > 0) await self.navigator.setAppBadge(count);
    else await self.navigator.clearAppBadge();
  } catch {
    // Badges are optional and may need notification permission.
  }
}

function sameOrigin(value) {
  try {
    const url = new URL(value, self.location.origin);
    if (url.origin === self.location.origin) return url;
  } catch {
    // Fall through to the app's start page.
  }
  return new URL("/app", self.location.origin);
}

// The server sends declarative push messages. Safari 18.4+ shows them itself;
// other browsers deliver them here.
self.addEventListener("push", (event) => {
  let message = {};
  try {
    message = event.data?.json() ?? {};
  } catch {
    // Every push must show a notification, even if its payload is unreadable.
  }
  const notification = message.notification ?? {};
  event.waitUntil(
    Promise.all([
      self.registration.showNotification(notification.title || "Keepsake", {
        body: notification.body ?? "",
        tag: notification.tag,
        lang: notification.lang,
        dir: notification.dir,
        icon: "/icons/icon-192.png",
        badge: "/icons/badge-96.png",
        data: { navigate: sameOrigin(notification.navigate ?? "/app").href },
      }),
      setBadge(notification.app_badge),
    ]),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target = sameOrigin(event.notification.data?.navigate ?? "/app");
  event.waitUntil(
    (async () => {
      const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
      const open = windows.find((client) => new URL(client.url).origin === self.location.origin);
      if (open) {
        // The page routes itself, so an unsaved answer can still ask before leaving.
        await open.focus();
        open.postMessage({ type: "keepsake:navigate", path: target.pathname + target.search });
        return;
      }
      await self.clients.openWindow(target.href);
    })(),
  );
});
