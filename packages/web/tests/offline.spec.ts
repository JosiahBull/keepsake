import type { Page } from "@playwright/test";
import { test, expect, type TestApp } from "./fixtures";
import { startExtraPractice } from "./actions";

const header = (page: Page) => page.getByRole("banner");
const tab = (page: Page, name: string) =>
  page.getByRole("navigation", { name: "Main navigation" }).getByRole("button", { name, exact: true });

async function reviews(app: TestApp, factId: number): Promise<{ total: number; items: { outcome: string }[] }> {
  return (await app.request.get(`/api/profiles/1/facts/${factId}/reviews`)).json();
}

async function exposures(app: TestApp): Promise<number> {
  const page = await (await app.request.get("/api/profiles/1/study/events?after=0&limit=100")).json();
  return page.items.filter((event: { kind: string }) => event.kind === "exposure").length;
}

async function practiceFact(app: TestApp, prompt = "Which synthetic answer waits for the network?") {
  return app.createFact({
    prompt,
    answer: "The offline answer",
    practices: [{ kind: "flashcard", prompt, answer: "The offline answer" }],
  });
}

test("extra practice carries on offline and sends its saves on reconnecting", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();

  await context.setOffline(true);
  await expect(header(page)).toContainText("Offline");
  await page.getByLabel("Your answer").fill("My offline attempt");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  // The exposure is kept on this device, which is enough to show the answer.
  await expect(page.getByText("The offline answer", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Good Correct", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  await expect(header(page)).toContainText("Offline · 2 saves to send");
  await expect(page.getByText("2 saves to send when you are back online.", { exact: true })).toBeVisible();
  await expect(tab(page, "Library")).toBeEnabled();
  expect((await reviews(app, fact.id)).total).toBe(0);

  await context.setOffline(false);
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  await expect(header(page)).not.toContainText("Offline");
  expect((await reviews(app, fact.id)).items).toMatchObject([{ outcome: "good" }]);
  expect(await exposures(app)).toBe(1);
});

test("saves made offline reach the server after the app is closed and opened again", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
  await context.setOffline(true);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await page.getByRole("button", { name: "Easy Correct, without effort", exact: true }).click();
  await expect(header(page)).toContainText("Offline · 2 saves to send");
  await page.close();

  await context.setOffline(false);
  const reopened = await context.newPage();
  await reopened.goto("/app");
  await expect(reopened.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
  await expect.poll(async () => (await reviews(app, fact.id)).items).toMatchObject([{ outcome: "easy" }]);
  expect(await exposures(app)).toBe(1);
});

test("a second open tab leaves the first tab's waiting saves alone", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await context.setOffline(true);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await page.getByRole("button", { name: "Hard Correct, with difficulty", exact: true }).click();
  await expect(header(page)).toContainText("2 saves to send");

  await context.setOffline(false);
  const other = await context.newPage();
  await other.goto("/app");
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  await expect.poll(async () => (await reviews(app, fact.id)).total).toBe(1);
  expect(await exposures(app)).toBe(1);
});

test("the header shows Syncing while a save takes a moment", async ({ app, page }) => {
  const fact = await practiceFact(app);
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route(/\/api\/profiles\/1\/reviews$/, async (route) => {
    await held;
    await route.continue();
  });
  try {
    await page.goto("/app");
    await startExtraPractice(page);
    await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(header(page).getByText("Syncing…", { exact: true })).toBeVisible();
    release();
    await expect(header(page).getByText("Syncing…", { exact: true })).toHaveCount(0);
    await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  } finally {
    release();
  }
});

test.describe("with the service worker", () => {
  // WebKit's test browser cannot reload a page offline through a service worker.
  test.skip(({ browserName }) => browserName === "webkit", "offline reload needs Chromium");
  test.use({ serviceWorkers: "allow" });

  async function installed(page: Page) {
    await page.goto("/app");
    await expect(page.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
    await page.evaluate(() => navigator.serviceWorker.ready);
    await expect.poll(() => page.evaluate(() => Boolean(navigator.serviceWorker.controller))).toBe(true);
  }

  test("the installed app opens its last profile offline", async ({ context, page }) => {
    await installed(page);
    await context.setOffline(true);
    await page.reload();
    await expect(page.getByLabel("Profile: Josiah", { exact: true })).toBeVisible();
    await expect(header(page)).toContainText("Offline");
    await context.setOffline(false);
  });

  test("turning offline use off removes this device's copy", async ({ context, page }) => {
    await installed(page);
    await page.getByLabel("Profile: Josiah", { exact: true }).click();
    await page.getByRole("button", { name: "Profile settings", exact: true }).click();
    const toggle = page.getByLabel("Use Keepsake offline on this device");
    await expect(toggle).toBeChecked();
    await toggle.uncheck();

    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole("alert")).toContainText("Could not load profiles.");
    await context.setOffline(false);
    await page.reload();
    await expect(page.getByLabel("Use Keepsake offline on this device")).not.toBeChecked();
  });
});
