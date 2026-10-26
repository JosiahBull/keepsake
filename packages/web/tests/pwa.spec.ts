import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures";

type Manifest = {
  id: string;
  start_url: string;
  scope: string;
  display: string;
  icons: { src: string; sizes: string; purpose: string }[];
  shortcuts: { url: string; icons: { src: string }[] }[];
  share_target: { action: string; method: string; params: Record<string, string> };
};

test("the document links an installable manifest whose icons and shortcuts resolve", async ({ app, page }) => {
  await page.goto("/");
  const head = page.locator("head");
  await expect(head.locator('link[rel="manifest"]')).toHaveAttribute("href", "/manifest.webmanifest");
  await expect(head.locator('link[rel="apple-touch-icon"]')).toHaveAttribute("href", "/icons/apple-touch-icon.png");
  await expect(head.locator('meta[name="viewport"]')).toHaveAttribute("content", /viewport-fit=cover/);

  const response = await app.request.get("/manifest.webmanifest");
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"]).toBe("application/manifest+json");
  const manifest = (await response.json()) as Manifest;
  expect(manifest).toMatchObject({ id: "/", start_url: "/", scope: "/", display: "standalone" });
  for (const [size, purpose] of [["192x192", "any"], ["512x512", "any"], ["192x192", "maskable"], ["512x512", "maskable"]]) {
    expect(manifest.icons, `${size} ${purpose}`).toContainEqual(expect.objectContaining({ sizes: size, purpose }));
  }
  expect(manifest.share_target).toEqual({
    action: "/share",
    method: "GET",
    params: { title: "title", text: "text", url: "url" },
  });
  const images = [
    "/icons/apple-touch-icon.png",
    "/icons/badge-96.png",
    ...manifest.icons.map((icon) => icon.src),
    ...manifest.shortcuts.flatMap((shortcut) => shortcut.icons.map((icon) => icon.src)),
  ];
  for (const src of new Set(images)) {
    const image = await app.request.get(src);
    expect(image.status(), src).toBe(200);
    expect(image.headers()["content-type"], src).toBe("image/png");
  }
  expect(manifest.shortcuts.map((shortcut) => shortcut.url)).toEqual([
    "/?open=practice",
    "/?open=add",
    "/?open=matching",
  ]);
});

test("Home Screen shortcuts open the remembered profile's screen", async ({ app, page }) => {
  await app.createFact({ prompt: "What is the shortcut fixture?", answer: "A scheduled fact" });
  await page.goto("/?open=practice");
  await expect(page).toHaveURL(/\/p\/1\/practice$/);
  await expect(page.getByRole("heading", { name: "What is the shortcut fixture?", exact: true })).toBeVisible();

  await page.goto("/?open=add");
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("region", { name: "Add a fact", exact: true })
    .getByRole("textbox", { name: "Question", exact: true })).toBeVisible();

  await page.goto("/?open=matching");
  await expect(page).toHaveURL(/\/p\/1\/matching$/);
});

test("a shared page opens the fact form with its title, text and address", async ({ page }) => {
  await page.goto("/share?" + new URLSearchParams({
    title: "Synthetic shared article",
    text: "A highlighted passage https://example.com/shared-article",
    url: "",
  }));
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  const form = page.getByRole("region", { name: "Add a fact", exact: true });
  await expect(form.getByRole("textbox", { name: "Context", exact: true }))
    .toHaveValue("Synthetic shared article\n\nA highlighted passage");
  await expect(form.getByRole("textbox", { name: "Source", exact: true }))
    .toHaveValue("https://example.com/shared-article");
  await expect(form.getByRole("textbox", { name: "Question", exact: true })).toHaveValue("");
  // The draft belongs to that one form; returning to the library does not reopen it.
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Library", exact: true }).click();
  await expect(page.getByRole("region", { name: "Add a fact", exact: true })).toHaveCount(0);
});

/**
 * Stand in for the browser's push service. Only the browser side is simulated; every
 * request still reaches the real Keepsake API, which accepts this HTTPS endpoint.
 */
async function simulatePush(page: Page) {
  await page.addInitScript(() => {
    const stored = "keepsake.test.subscription";
    let permission: NotificationPermission = "default";
    const subscription = (key: ArrayBuffer) => ({
      endpoint: "https://push.example/keepsake-browser-test",
      options: { applicationServerKey: key },
      toJSON: () => ({
        endpoint: "https://push.example/keepsake-browser-test",
        keys: {
          p256dh: "BLn9b-VR0ca83knDNZ32dCHGyjJp-1riX9ZTN40MqV8K_LpQmLqxC_DoHvqvFXO_nGdAB4W9dogZb_sM-uV4JbY",
          auth: "_ordMnz7uTCmrpBTeUV4Bw",
        },
      }),
      unsubscribe: async () => {
        localStorage.removeItem(stored);
        return true;
      },
    });
    const pushManager = {
      getSubscription: async () => {
        const key = localStorage.getItem(stored);
        return key ? subscription(Uint8Array.from(JSON.parse(key) as number[]).buffer) : null;
      },
      subscribe: async ({ applicationServerKey }: { applicationServerKey: Uint8Array }) => {
        localStorage.setItem(stored, JSON.stringify([...applicationServerKey]));
        return subscription(applicationServerKey.buffer as ArrayBuffer);
      },
    };
    const registration = { pushManager };
    Object.defineProperty(window, "PushManager", { value: function PushManager() {}, configurable: true });
    Object.defineProperty(navigator, "serviceWorker", {
      configurable: true,
      value: {
        register: async () => registration,
        ready: Promise.resolve(registration),
        addEventListener() {},
        removeEventListener() {},
      },
    });
    Object.defineProperty(window, "Notification", {
      configurable: true,
      value: {
        get permission() {
          return localStorage.getItem(stored) ? "granted" : permission;
        },
        requestPermission: async () => (permission = "granted"),
      },
    });
  });
}

test("reminders can be turned on, timed and turned off for this device", async ({ app, page }) => {
  await simulatePush(page);
  await page.goto("/p/1/settings");
  const reminders = page.getByRole("region", { name: "Reminders", exact: true });
  await expect(reminders.getByText("Reminders are off for this device.", { exact: true })).toBeVisible();
  await expect(reminders.getByLabel("Earliest reminder")).toHaveCount(0);

  await reminders.getByRole("button", { name: "Turn on reminders", exact: true }).click();
  await expect(reminders.getByText("Reminders are on for this device.", { exact: true })).toBeVisible();
  await expect(reminders.getByRole("button", { name: "Send a test", exact: true })).toBeEnabled();
  const saved = await app.request.get("/api/profiles/1/reminders");
  expect(await saved.json()).toMatchObject({ devices: 1, minute_of_day: 480, timezone: "Europe/London" });

  const time = reminders.getByLabel("Earliest reminder");
  await expect(time).toHaveValue("08:00");
  const save = reminders.getByRole("button", { name: "Save time", exact: true });
  await expect(save).toBeDisabled();
  await time.fill("18:30");
  await save.click();
  await expect(reminders.getByRole("status")).toHaveText("Reminder time saved.");
  expect(await (await app.request.get("/api/profiles/1/reminders")).json()).toMatchObject({ minute_of_day: 1110 });

  // Reopening re-registers the stored subscription with the server.
  await app.request.delete("/api/profiles/1/reminders/devices", {
    data: { endpoint: "https://push.example/keepsake-browser-test" },
  });
  const registered = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/profiles/1/reminders/devices" &&
    response.request().method() === "PUT");
  await page.reload();
  expect((await registered).ok()).toBe(true);
  await expect(reminders.getByText("Reminders are on for this device.", { exact: true })).toBeVisible();
  await expect(reminders.getByLabel("Earliest reminder")).toHaveValue("18:30");
  expect(await (await app.request.get("/api/profiles/1/reminders")).json()).toMatchObject({ devices: 1 });

  await reminders.getByRole("button", { name: "Turn off", exact: true }).click();
  await expect(reminders.getByText("Reminders are off for this device.", { exact: true })).toBeVisible();
  expect(await (await app.request.get("/api/profiles/1/reminders")).json()).toMatchObject({ devices: 0 });
  await expect(reminders.getByLabel("Earliest reminder")).toHaveCount(0);
});

test("Safari explains that reminders need the Home Screen app", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "mobile-webkit", "iPhone guidance depends on the iPhone user agent");
  await page.addInitScript(() => {
    // Safari exposes Web Push only to Home Screen web apps.
    delete (window as { PushManager?: unknown }).PushManager;
  });
  await page.goto("/p/1/settings");
  const reminders = page.getByRole("region", { name: "Reminders", exact: true });
  await expect(reminders).toContainText("tap Share, then Add to Home Screen");
  await expect(reminders.getByRole("button")).toHaveCount(0);
});
