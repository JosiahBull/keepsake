import { test, expect } from "./fixtures";
import { startExtraPractice } from "./actions";

const securityHeaders = {
  "x-content-type-options": "nosniff",
  "referrer-policy": "no-referrer",
  "x-frame-options": "DENY",
  "cross-origin-opener-policy": "same-origin",
};

function expectResponsePolicy(headers: Record<string, string>, cache: string) {
  expect(headers).toMatchObject({
    ...securityHeaders,
    "cache-control": cache,
    "cdn-cache-control": cache,
    "cloudflare-cdn-cache-control": cache,
  });
  expect(headers["x-request-id"]).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
}

test("empty practice offers a working path to the fact form", async ({ app, page }) => {
  await page.goto("/app");
  await expect(page.getByText("No facts yet. Add a fact to start.", { exact: true })).toBeVisible();
  await startExtraPractice(page);

  const practice = page.getByRole("region", { name: "Practice session", exact: true });
  await expect(practice.getByRole("heading", { name: "No facts yet.", exact: true })).toBeVisible();
  await expect(practice.getByRole("button", { name: /^(Reveal answer|Check answer|Again\b|Hard\b|Good\b|Easy\b)/ })).toHaveCount(0);
  await expect(practice.getByRole("alert")).toHaveCount(0);
  await practice.getByRole("button", { name: "Add a fact", exact: true }).click();

  await expect(page.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
  const form = page.getByRole("region", { name: "Add a fact", exact: true });
  await expect(form.getByRole("textbox", { name: "Question", exact: true })).toBeVisible();
  await expect(form.getByRole("textbox", { name: "Answer", exact: true })).toBeVisible();
  await expect(form.getByRole("button", { name: "Save fact", exact: true })).toBeEnabled();
  const facts = await app.request.get("/api/profiles/1/facts");
  expect(facts.ok()).toBe(true);
  expect(await facts.json()).toMatchObject({ total: 0, items: [] });
});

test("matching requires three distinct pairs within a topic bank even when duplicate records make three facts", async ({ app, page }) => {
  await page.goto("/app");
  const start = page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Match pairs", exact: true });
  const empty = page.getByRole("heading", {
    name: "Add three related facts with a shared topic tag to play.",
    exact: true,
  });
  await start.click();
  await expect(empty).toBeVisible();
  await expect(page.getByRole("region", { name: "Matching pairs game" })).toHaveCount(0);

  for (const fact of [
    { prompt: "Synthetic household pair one?", answer: "Synthetic answer one" },
    { prompt: "Synthetic household pair one?", answer: "Synthetic answer one" },
    { prompt: "Synthetic household pair two?", answer: "Synthetic answer two" },
  ]) {
    await app.createFact({ ...fact, tags: ["synthetic-household-bank"] });
  }
  const listed = await app.request.get("/api/profiles/1/facts");
  expect(listed.ok()).toBe(true);
  expect((await listed.json()).total).toBe(3);

  const sample = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/profiles/1/facts/sample");
  await start.click();
  const response = await sample;
  expect(response.ok()).toBe(true);
  const pairs = await response.json();
  expect(pairs.items).toEqual([]);
  await expect(empty).toBeVisible();
  await expect(page.getByRole("heading", { name: "Rate your recognition", exact: true })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Matching pairs game" })).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Practise", exact: true })).toHaveCount(2);
});

test("untagged facts from different topics do not form a matching bank", async ({ app, page }) => {
  for (const [prompt, answer] of [
    ["Synthetic food question?", "Synthetic food answer"],
    ["Synthetic Cargo question?", "Synthetic Cargo answer"],
    ["Synthetic astronomy question?", "Synthetic astronomy answer"],
    ["Synthetic gardening question?", "Synthetic gardening answer"],
  ]) {
    await app.createFact({ prompt, answer, tags: [] });
  }
  await page.goto("/app");
  const sample = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/profiles/1/facts/sample");
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Match pairs", exact: true }).click();
  const response = await sample;
  expect(response.ok()).toBeTruthy();
  expect(await response.json()).toEqual({ items: [] });
  await expect(page.getByRole("heading", {
    name: "Add three related facts with a shared topic tag to play.",
    exact: true,
  })).toBeVisible();
  await expect(page.getByRole("region", { name: "Matching pairs game" })).toHaveCount(0);
});

test("the production document, browser-loaded assets and API enforce their response policies", async ({ page }) => {
  const scriptLoaded = page.waitForResponse((response) =>
    /^\/assets\/[^/]+\.js$/.test(new URL(response.url()).pathname));
  const libraryLoaded = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/profiles/1/facts");
  const document = await page.goto("/app");
  expect(document).not.toBeNull();
  expect(document!.status()).toBe(200);
  expect(document!.headers()["content-type"]).toContain("text/html");
  expectResponsePolicy(document!.headers(), "public, max-age=0, must-revalidate");

  const script = await scriptLoaded;
  expect(script.status()).toBe(200);
  expect(script.headers()["content-type"]).toMatch(/^(application|text)\/javascript\b/);
  expectResponsePolicy(script.headers(), "public, max-age=31536000, immutable");
  expect(await script.text()).not.toMatch(/^\s*<!doctype html>/i);
  await expect(page.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
  await expect(page.getByText("No facts yet. Add a fact to start.", { exact: true })).toBeVisible();

  const library = await libraryLoaded;
  expect(library.status()).toBe(200);
  expect(library.headers()["content-type"]).toContain("application/json");
  expectResponsePolicy(library.headers(), "no-store");
  expect(await library.json()).toMatchObject({ total: 0, items: [] });

  // These requests originate inside the browser and use the same live Rust server.
  const missing = await page.evaluate(async () => {
    return Promise.all(["/assets/absent-e2e-module.js", "/api/absent-e2e-route"].map(async (url) => {
      const response = await fetch(url);
      return {
        url,
        status: response.status,
        headers: Object.fromEntries(response.headers),
        body: await response.text(),
      };
    }));
  });
  for (const response of missing) {
    expect(response.status, response.url).toBe(404);
    expectResponsePolicy(response.headers, "no-store");
  }
  const apiError = missing.find((response) => response.url.startsWith("/api/"))!;
  expect(apiError.headers["content-type"]).toContain("application/json");
  expect(JSON.parse(apiError.body)).toEqual({ error: "Not Found" });
});
