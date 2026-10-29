import { test, expect } from "./fixtures";
import { startExtraPractice } from "./actions";

test("empty library and fact form", async ({ page }) => {
  await page.goto("/app");
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
  await page.goto("/app");
  await page.getByRole("heading", { name: fact.prompt, exact: true }).click();
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  await expect(page.getByText("Saving…", { exact: true })).toHaveCount(0);
  await expect.soft(page).toHaveScreenshot("library-fact.png", { fullPage: true });

  await startExtraPractice(page);
  await page.getByLabel("Your answer").fill("Water");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Rate your attempt", exact: true })).toBeVisible();
  await expect.soft(page).toHaveScreenshot("practice-feedback.png", { fullPage: true });
});

test("scheduled recall feedback shows its interval previews", async ({ app, page }, testInfo) => {
  const fact = await app.createFact({
    prompt: "What does wai mean?",
    answer: "Water",
    context: "He wai māu? Would you like some water?",
    source: "Synthetic household notebook",
    tags: ["te reo", "household"],
  });
  await page.goto("/app");
  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await page.getByRole("heading", { name: fact.prompt, exact: true }).click();
  await page.getByLabel("Your answer").fill("Water");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await expect(page.getByRole("button", { name: /^Good\b/ })).toBeEnabled();
  await expect(page.getByRole("button", { name: /^Again\b/ })).toContainText("10 min");
  await page.screenshot({ path: testInfo.outputPath("scheduled-feedback-review.png"), fullPage: true });
  await expect(page).toHaveScreenshot("scheduled-feedback.png", { fullPage: true });
});

test.describe("profile screenshots", () => {
  test.use({ profileSetup: false });
  test("household chooser, share confirmation and recipient inbox", async ({ app, page }, testInfo) => {
    const renamed = await app.request.patch("/api/profiles/1", { data: { name: "Josiah" } });
    expect(renamed.ok()).toBe(true);
    const created = await app.request.post("/api/profiles", { data: { name: "Partner", handle: "partner" } });
    expect(created.status()).toBe(201);
    const partner = await created.json();
    const fact = await app.createFact({ prompt: "What does wai mean?", answer: "Water", tags: ["te reo", "household"] });
    await page.goto("/app");
    await expect(page.getByRole("heading", { name: "Who’s practising?", exact: true })).toBeVisible();
    // First-entry screenshots must include the decoded raster logo, especially in WebKit.
    await page.getByRole("img", { name: "Keepsake", exact: true }).evaluate(async (logo: HTMLImageElement) => {
      await logo.decode();
      if (!logo.complete || logo.naturalWidth === 0) throw new Error("The Keepsake logo did not load.");
    });
    await page.screenshot({ path: testInfo.outputPath("profile-chooser-review.png"), fullPage: true });
    await expect.soft(page).toHaveScreenshot("profile-chooser.png", { fullPage: true });
    await page.getByRole("button", { name: "Josiah", exact: true }).click();
    await page.getByRole("button", { name: "Select", exact: true }).click();
    await page.getByRole("checkbox", { name: `Select ${fact.prompt}`, exact: true }).check();
    await page.getByRole("button", { name: "Share selected", exact: true }).click();
    await page.getByLabel("Share with", { exact: true }).selectOption(String(partner.id));
    await page.screenshot({ path: testInfo.outputPath("share-facts-review.png"), fullPage: true });
    await expect.soft(page).toHaveScreenshot("share-facts.png", { fullPage: true });
    await page.getByRole("button", { name: "Send facts", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Facts shared." })).toHaveText("Facts shared.");
    await page.getByLabel("Profile: Josiah", { exact: true }).click();
    await page.getByRole("button", { name: "Partner", exact: true }).click();
    await page.getByRole("button", { name: "Shared with me", exact: true }).click();
    await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("shared-inbox-review.png"), fullPage: true });
    await expect.soft(page).toHaveScreenshot("shared-inbox.png", { fullPage: true });
  });
});

test("install page", async ({ page, isMobile }) => {
  // The written steps are the baseline, not a prompt the test browser might offer.
  await page.addInitScript(() => {
    addEventListener("beforeinstallprompt", (event) => event.stopImmediatePropagation(), { capture: true });
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Keep what you learn", level: 1 })).toBeVisible();
  await page.getByRole("img", { name: "Keepsake", exact: true }).evaluate(async (logo: HTMLImageElement) => {
    await logo.decode();
  });
  // The QR code encodes the test server's port, which changes on every run.
  const qr = page.getByRole("img", { name: /^QR code/ });
  await expect.soft(page).toHaveScreenshot("install.png", { fullPage: true, mask: [qr] });
  if (isMobile) {
    await page.getByRole("button", { name: "Install Keepsake", exact: true }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await expect.soft(page).toHaveScreenshot("install-steps.png");
  }
});
