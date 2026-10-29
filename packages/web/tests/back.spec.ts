import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures";

/** Reports the iOS Home Screen app, the only place Keepsake handles the edge swipe itself. */
async function homeScreenApp(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "standalone", { configurable: true, get: () => true });
  });
}

/**
 * Drags right from `fromX` with touch-type pointer events, pausing between moves.
 * Test browsers cannot send real touch drags to every engine.
 */
async function swipeRight(page: Page, fromX: number, distance: number, pause = 0) {
  const main = page.getByRole("main");
  const at = (step: number) => ({
    pointerId: 9,
    pointerType: "touch",
    isPrimary: true,
    clientX: fromX + (distance * step) / 8,
    clientY: 300,
  });
  await main.dispatchEvent("pointerdown", at(0));
  for (let step = 1; step <= 8; step += 1) {
    if (pause) await page.waitForTimeout(pause);
    await main.dispatchEvent("pointermove", at(step));
  }
  await main.dispatchEvent("pointerup", at(8));
}

async function openPractice(page: Page) {
  await page.goto("/p/1/library");
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/practice$/);
}

/** Waits long enough for a swipe that was going to go back to have done so. */
async function settled(page: Page) {
  await page.waitForTimeout(400);
  await expect(page.getByRole("main")).toHaveCSS("transform", "none");
}

test("in the Home Screen app, swiping in from the left edge goes back, but never out of Keepsake", async ({
  page,
  isMobile,
}) => {
  test.skip(!isMobile, "The edge swipe is for touch screens.");
  await homeScreenApp(page);
  await openPractice(page);
  await swipeRight(page, 4, 250);
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("heading", { name: "Library", level: 1 })).toBeVisible();
  await settled(page);

  // This is the first screen Keepsake showed, so there is nothing of Keepsake's to go back to.
  await swipeRight(page, 4, 250);
  await settled(page);
  await expect(page).toHaveURL(/\/p\/1\/library$/);
});

test("a short, slow swipe or one away from the edge leaves the page where it is", async ({ page, isMobile }) => {
  test.skip(!isMobile, "The edge swipe is for touch screens.");
  await homeScreenApp(page);
  await openPractice(page);

  await swipeRight(page, 4, 60, 40);
  await settled(page);
  await expect(page).toHaveURL(/\/p\/1\/practice$/);

  await swipeRight(page, 120, 250);
  await settled(page);
  await expect(page).toHaveURL(/\/p\/1\/practice$/);
});

test("Safari keeps its own edge swipe, so the page does not add one", async ({ page, isMobile }) => {
  test.skip(!isMobile, "The edge swipe is for touch screens.");
  await openPractice(page);
  await swipeRight(page, 4, 250);
  await settled(page);
  await expect(page).toHaveURL(/\/p\/1\/practice$/);
});

test("an edge swipe away from unsaved settings asks before discarding them", async ({ page, isMobile }) => {
  test.skip(!isMobile, "The edge swipe is for touch screens.");
  await homeScreenApp(page);
  await page.goto("/p/1/library");
  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await page.getByRole("button", { name: "Profile settings", exact: true }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill("Josiah B");

  await swipeRight(page, 4, 250);
  const discard = page.getByRole("alertdialog");
  await expect(discard).toBeVisible();
  await discard.getByRole("button", { name: "Keep working", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/settings$/);
  await expect(page.getByRole("textbox", { name: "Name", exact: true })).toHaveValue("Josiah B");
});

test("on phones, Profile settings has a back button to the screen it was opened from", async ({ page, isMobile }) => {
  test.skip(!isMobile, "Wider screens keep the navigation in view instead.");
  await openPractice(page);
  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await page.getByRole("button", { name: "Profile settings", exact: true }).click();
  await page.getByRole("button", { name: "Back to Practise", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/practice$/);

  // Opened directly, settings goes back to the library.
  await page.goto("/p/1/settings");
  await page.getByRole("button", { name: "Back to Library", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("heading", { name: "Library", level: 1 })).toBeVisible();
});
