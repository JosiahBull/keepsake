import { test, expect } from "./fixtures";

// Push delivery uses the Chromium DevTools protocol. Chromium's headless shell denies
// notifications; its new headless mode supports them.
test.skip(({ browserName }) => browserName !== "chromium", "push delivery needs Chromium");
test.use({ serviceWorkers: "allow", channel: "chromium" });

test("shows pushed reminders and opens the cached shell offline", async ({ app, context, page }) => {
  await context.grantPermissions(["notifications"], { origin: app.url });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
  await page.evaluate(() => navigator.serviceWorker.ready);
  await expect.poll(() => page.evaluate(() => Boolean(navigator.serviceWorker.controller))).toBe(true);

  const worker = await app.request.get("/sw.js");
  expect(worker.headers()["content-type"]).toMatch(/^(application|text)\/javascript\b/);
  expect(worker.headers()["cache-control"]).toBe("public, max-age=0, must-revalidate");

  const cdp = await context.newCDPSession(page);
  const registrationId = new Promise<string>((resolve) => {
    cdp.on("ServiceWorker.workerRegistrationUpdated", ({ registrations }) => {
      const registration = registrations.find((item) => item.scopeURL === `${app.url}/` && !item.isDeleted);
      if (registration) resolve(registration.registrationId);
    });
  });
  await cdp.send("ServiceWorker.enable");
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: app.url,
    registrationId: await registrationId,
    data: JSON.stringify({
      web_push: 8030,
      notification: {
        title: "Time to practise, Josiah",
        body: "2 reviews are ready.",
        navigate: "https://elsewhere.example/p/1/practice",
        lang: "en",
        dir: "ltr",
        tag: "reminder-1",
        app_badge: "2",
      },
    }),
  });
  await expect.poll(() => page.evaluate(async () => {
    const notifications = await (await navigator.serviceWorker.ready).getNotifications();
    return notifications.map(({ title, body, tag, data }) => ({ title, body, tag, data }));
  })).toEqual([{
    title: "Time to practise, Josiah",
    body: "2 reviews are ready.",
    tag: "reminder-1",
    // Links to another origin are replaced by the app's own start page.
    data: { navigate: `${app.url}/` },
  }]);

  await context.setOffline(true);
  await page.reload();
  await expect(page.getByRole("alert")).toContainText("Could not load profiles.");
  await page.getByRole("img", { name: "Keepsake", exact: true }).evaluate(async (logo: HTMLImageElement) => {
    await logo.decode();
  });
  await context.setOffline(false);
});
