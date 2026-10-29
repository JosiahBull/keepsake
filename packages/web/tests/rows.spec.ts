import type { Locator, Page } from "@playwright/test";
import { test, expect, type TestApp } from "./fixtures";

const prompts = ["What does wai mean?", "What does kai mean?", "What does whare mean?"];

async function threeFacts(app: TestApp) {
  const facts = [];
  for (const prompt of prompts) {
    facts.push(await app.createFact({ prompt, answer: `Answer for ${prompt}` }));
  }
  return facts;
}

function row(page: Page, prompt: string): Locator {
  return page.getByRole("article").filter({ has: page.getByRole("heading", { name: prompt, exact: true }) });
}

/** Drags a row with touch-type pointer events. Test browsers cannot send real touch drags to every engine. */
async function drag(target: Locator, dx: number, dy = 0) {
  const box = (await target.boundingBox())!;
  const x = box.x + box.width - 40;
  const y = box.y + box.height / 2;
  const at = (step: number) => ({
    pointerId: 7,
    pointerType: "touch",
    isPrimary: true,
    clientX: x + (dx * step) / 6,
    clientY: y + (dy * step) / 6,
  });
  await target.dispatchEvent("pointerdown", at(0));
  for (let step = 1; step <= 6; step += 1) await target.dispatchEvent("pointermove", at(step));
  await target.dispatchEvent("pointerup", at(6));
}

/** How far the row's content has slid from its resting place. */
async function slid(fact: Locator): Promise<number> {
  const content = await fact.locator("summary").boundingBox();
  const resting = await fact.boundingBox();
  return Math.round(content!.x - resting!.x);
}

/** Shows a row's actions: touch screens swipe the row, pointers hover it. */
async function revealActions(fact: Locator, isMobile: boolean) {
  if (isMobile) {
    await drag(fact.locator("summary"), -120);
    await expect.poll(() => slid(fact)).toBe(-152);
  } else {
    await fact.hover();
  }
}

test("swiping a row left uncovers Share and Delete, and a tap closes it without opening the fact", async ({
  app,
  page,
  isMobile,
}) => {
  test.skip(!isMobile, "Rows slide on touch screens; pointers hover instead.");
  await threeFacts(app);
  await page.goto("/");
  const kai = row(page, "What does kai mean?");
  await expect(kai).toBeVisible();

  // A short swipe springs back, and so does a mostly vertical drag, which scrolls instead.
  await drag(kai.locator("summary"), -50);
  await expect.poll(() => slid(kai)).toBe(0);
  await drag(kai.locator("summary"), -20, 80);
  await expect.poll(() => slid(kai)).toBe(0);

  await drag(kai.locator("summary"), -120);
  await expect.poll(() => slid(kai)).toBe(-152);
  await expect(kai.getByRole("button", { name: "Share What does kai mean?", exact: true })).toBeVisible();
  await expect(kai.getByRole("button", { name: "Delete What does kai mean?", exact: true })).toBeVisible();

  // Swiping another row closes the first.
  const wai = row(page, "What does wai mean?");
  await drag(wai.locator("summary"), -120);
  await expect.poll(() => slid(wai)).toBe(-152);
  await expect.poll(() => slid(kai)).toBe(0);

  await wai.locator("summary").click();
  await expect.poll(() => slid(wai)).toBe(0);
  await expect(wai.getByText("Answer for What does wai mean?", { exact: true })).toHaveCount(0);
  await wai.locator("summary").click();
  await expect(wai.getByText("Answer for What does wai mean?", { exact: true })).toBeVisible();
});

test("rows leave sideways drags to the page while it is pinch-zoomed", async ({ app, page, isMobile }) => {
  test.skip(!isMobile, "Rows slide on touch screens; pointers hover instead.");
  await threeFacts(app);
  await page.goto("/");
  const kai = row(page, "What does kai mean?");
  await expect(kai).toBeVisible();
  // Test browsers cannot pinch, so the visual viewport reports a zoomed scale instead.
  await page.evaluate(() => {
    Object.defineProperty(visualViewport!, "scale", { configurable: true, get: () => 2 });
    visualViewport!.dispatchEvent(new Event("resize"));
  });
  await expect(kai.locator(".row-content")).toHaveCSS("touch-action", "manipulation");
  await drag(kai.locator("summary"), -120);
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(resolve)));
  expect(await slid(kai)).toBe(0);

  await page.evaluate(() => {
    Object.defineProperty(visualViewport!, "scale", { configurable: true, get: () => 1 });
    visualViewport!.dispatchEvent(new Event("resize"));
  });
  await expect(kai.locator(".row-content")).toHaveCSS("touch-action", "pan-y");
  await drag(kai.locator("summary"), -120);
  await expect.poll(() => slid(kai)).toBe(-152);
});

test("a row's Share offers just that fact", async ({ app, page, isMobile }) => {
  const created = await app.request.post("/api/profiles", { data: { name: "Partner", handle: "partner" } });
  expect(created.status()).toBe(201);
  const partner = (await created.json()) as { id: number };
  const facts = await threeFacts(app);
  await page.goto("/");
  const kai = row(page, "What does kai mean?");
  await revealActions(kai, isMobile);
  await kai.getByRole("button", { name: "Share What does kai mean?", exact: true }).click();

  await expect(page.getByRole("heading", { name: "Share 1 fact", exact: true })).toBeVisible();
  await page.getByLabel("Share with", { exact: true }).selectOption(String(partner.id));
  await page.getByRole("button", { name: "Send facts", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Facts shared." })).toHaveText("Facts shared.");
  const offers = await (await app.request.get(`/api/profiles/${partner.id}/shares?direction=inbox`)).json();
  expect(offers.items.map((offer: { source_fact_id: number }) => offer.source_fact_id)).toEqual([facts[1].id]);
});

test("Delete asks first; Cancel keeps the fact and Delete fact removes it everywhere", async ({
  app,
  page,
  isMobile,
}) => {
  const facts = await threeFacts(app);
  await page.goto("/");
  await expect(page.getByText("3 facts", { exact: true })).toBeVisible();
  const kai = row(page, "What does kai mean?");

  await revealActions(kai, isMobile);
  await kai.getByRole("button", { name: "Delete What does kai mean?", exact: true }).click();
  const dialog = page.getByRole("alertdialog", { name: "Delete this fact?" });
  await expect(dialog).toContainText("What does kai mean?");
  await expect(dialog.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("article")).toHaveCount(3);
  expect((await app.request.get(`/api/profiles/1/facts/${facts[1].id}`)).status()).toBe(200);

  await revealActions(kai, isMobile);
  await kai.getByRole("button", { name: "Delete What does kai mean?", exact: true }).click();
  await dialog.getByRole("button", { name: "Delete fact", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("status").filter({ hasText: "Fact deleted." })).toHaveText("Fact deleted.");
  await expect(kai).toHaveCount(0);
  await expect(page.getByRole("article")).toHaveCount(2);
  await expect(page.getByText("2 facts", { exact: true })).toBeVisible();
  expect((await app.request.get(`/api/profiles/1/facts/${facts[1].id}`)).status()).toBe(404);

  await page.reload();
  await expect(page.getByRole("article").getByRole("heading")).toHaveText(["What does whare mean?", "What does wai mean?"]);
});

test("a failed delete keeps the fact and its dialog, and a fact deleted elsewhere still leaves the list", async ({
  app,
  page,
  isMobile,
}) => {
  const facts = await threeFacts(app);
  await page.route(`**/api/profiles/1/facts/${facts[0].id}`, async (route) => {
    if (route.request().method() !== "DELETE") {
      await route.continue();
      return;
    }
    await route.fulfill({ status: 503, json: { error: "The library is busy." } });
  });
  await page.goto("/");
  const wai = row(page, "What does wai mean?");
  await revealActions(wai, isMobile);
  await wai.getByRole("button", { name: "Delete What does wai mean?", exact: true }).click();
  const dialog = page.getByRole("alertdialog", { name: "Delete this fact?" });
  await dialog.getByRole("button", { name: "Delete fact", exact: true }).click();
  await expect(dialog.getByRole("alert")).toHaveText("Could not delete the fact. The library is busy.");
  await expect(page.getByRole("article")).toHaveCount(3);

  // Meanwhile the CLI deletes it. The retry's 404 means it is gone, so the list drops it.
  await page.unroute(`**/api/profiles/1/facts/${facts[0].id}`);
  await app.cli(["delete", String(facts[0].id)]);
  await dialog.getByRole("button", { name: "Delete fact", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(wai).toHaveCount(0);
  await expect(page.getByText("2 facts", { exact: true })).toBeVisible();
});

test("keyboard focus reaches a row's actions and shows them", async ({ app, page, browserName, isMobile }) => {
  test.skip(browserName === "webkit", "Safari's button tab stops depend on the user's keyboard setting.");
  await threeFacts(app);
  await page.goto("/");
  const kai = row(page, "What does kai mean?");
  await kai.locator("summary").focus();
  await page.keyboard.press("Tab");
  const share = kai.getByRole("button", { name: "Share What does kai mean?", exact: true });
  await expect(share).toBeFocused();
  if (isMobile) await expect.poll(() => slid(kai)).toBe(-152);
  else await expect(kai.locator(".row-actions")).toHaveCSS("opacity", "1");
  await page.keyboard.press("Tab");
  await expect(kai.getByRole("button", { name: "Delete What does kai mean?", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  if (isMobile) await expect.poll(() => slid(kai)).toBe(0);
  else await expect(kai.locator(".row-actions")).toHaveCSS("opacity", "0");
});

test("Select turns rows into a checklist for sharing, and Done turns them back", async ({ app, page }) => {
  await threeFacts(app);
  await page.goto("/");
  await expect(page.getByRole("checkbox")).toHaveCount(0);
  await page.getByRole("button", { name: "Select", exact: true }).click();

  const toolbar = page.getByRole("toolbar", { name: "Selected facts" });
  await expect(toolbar).toContainText("Choose facts to share");
  await expect(toolbar.getByRole("button", { name: "Share selected", exact: true })).toBeDisabled();
  await expect(page.getByRole("checkbox")).toHaveCount(3);
  await expect(page.getByRole("article").locator("summary")).toHaveCount(0);

  // The whole row is the checkbox's label.
  await page.getByText("What does kai mean?", { exact: true }).click();
  await page.getByRole("checkbox", { name: "Select What does wai mean?", exact: true }).check();
  await expect(toolbar).toContainText("2 selected");
  await toolbar.getByRole("button", { name: "Share selected", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Share 2 facts", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();

  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.getByRole("checkbox")).toHaveCount(0);
  await expect(toolbar).toHaveCount(0);
  await expect(page.getByRole("article").locator("summary")).toHaveCount(3);
});
