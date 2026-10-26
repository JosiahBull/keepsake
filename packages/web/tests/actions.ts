import type { Page } from "@playwright/test";

export async function startExtraPractice(page: Page) {
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  await page.getByRole("button", { name: "Extra practice", exact: true }).click();
}

/** Sounds requested since the page loaded, in order. */
export function sounds(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { keepsakeSounds: string[] }).keepsakeSounds);
}
