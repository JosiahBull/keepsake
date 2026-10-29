// Reminders, app badges and an offline app shell. API responses are never cached.
const SHELL = "keepsake-shell-v1";
const PRECACHE = ["/keepsake-logo.png"];

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
        if (key !== SHELL) await caches.delete(key);
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
    if (new URL(request.url).pathname === "/") {
      // Caching must not delay or fail the launch it is preparing for.
      event.waitUntil(storeShell(response.clone()).catch(() => undefined));
    }
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
