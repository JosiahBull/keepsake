import { test, expect } from "./fixtures";

test("empty library and fact form", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("No facts yet. Add a fact to start.")).toBeVisible();
  await expect.soft(page).toHaveScreenshot("empty-library.png", { fullPage: true });

  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByLabel("Question", { exact: true }).fill("What does wai mean?");
  await page.getByLabel("Answer", { exact: true }).fill("Water");
  await page.getByText("Context, source and tags (optional)", { exact: true }).click();
  await page.getByLabel("Context", { exact: true }).fill("He wai māu? Would you like some water?");
  await page.getByLabel("Source", { exact: true }).fill("Synthetic household notebook");
  await page.getByLabel("Tags").fill("te reo, household");
  await page.getByRole("heading", { name: "Add a fact", exact: true }).click();
  await expect.soft(page).toHaveScreenshot("fact-form.png", { fullPage: true });
});

test("expanded library fact and practice feedback", async ({ app, page }) => {
  const fact = await app.createFact({
    prompt: "What does wai mean?",
    answer: "Water",
    context: "He wai māu? Would you like some water?",
    source: "Synthetic household notebook",
    tags: ["te reo", "household"],
  });
  await page.goto("/");
  await page.getByRole("heading", { name: fact.prompt, exact: true }).click();
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  await expect.soft(page).toHaveScreenshot("library-fact.png", { fullPage: true });

  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await page.getByLabel("Your answer").fill("Water");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Rate your attempt", exact: true })).toBeVisible();
  await expect.soft(page).toHaveScreenshot("practice-feedback.png", { fullPage: true });
});
