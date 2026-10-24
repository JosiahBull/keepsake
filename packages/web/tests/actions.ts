import type { Page } from "@playwright/test";

export async function startExtraPractice(page: Page) {
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  await page.getByRole("button", { name: "Extra practice", exact: true }).click();
}
