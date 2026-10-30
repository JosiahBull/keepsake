import type { Page, Route } from "@playwright/test";
import { test, expect } from "./fixtures";
import { startExtraPractice } from "./actions";

function gate() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });

  return { promise, release };
}

async function unavailable(route: Route) {
  await route.fulfill({
    status: 503,
    contentType: "application/json",
    headers: { "Retry-After": "1" },
    body: JSON.stringify({ error: "Temporarily unavailable." }),
  });
}

async function expectNavigationDisabled(page: Page) {
  for (const name of ["Library", "Practise", "Match pairs"]) {
    await expect(page.getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name, exact: true })).toBeDisabled();
  }
  await expect(page.getByRole("banner")
    .getByRole("button", { name: "Add a fact", exact: true })).toBeDisabled();
}

test("library retries a failed load and shows loading until the real response arrives", async ({ app, page }) => {
  const fact = await app.createFact({ prompt: "Recovery library question" });
  const retry = gate();
  let requests = 0;

  await page.route(/\/api\/profiles\/1\/facts\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    await expect(page.getByRole("alert")).toContainText("Could not load your library.");
    await expect(page.getByText("No facts yet. Add a fact to start.")).toBeHidden();
    await page.getByRole("button", { name: "Try again", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Loading library…");
    await expect(page.getByRole("alert")).toBeHidden();
    retry.release();

    await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
    await expect(page.getByText("Loading library…", { exact: true })).toBeHidden();
    expect(requests).toBe(2);
  } finally {
    retry.release();
  }
});

test("a loading screen shows its mark in the middle of the screen without moving the page", async ({ page }) => {
  const held = gate();
  await page.route(/\/api\/profiles\/1\/facts\?/, async (route) => {
    await held.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    const status = page.getByRole("status").filter({ hasText: "Loading library…" });
    await expect(status).toHaveText("Loading library…");
    const tabs = await page.getByRole("button", { name: "My library", exact: true }).boundingBox();
    const mark = await status.locator("xpath=following-sibling::*[1]").boundingBox();
    const viewport = page.viewportSize()!;
    expect(Math.abs(mark!.x + mark!.width / 2 - viewport.width / 2)).toBeLessThan(2);
    expect(Math.abs(mark!.y + mark!.height / 2 - viewport.height / 2)).toBeLessThan(40);
    held.release();
    await expect(status).toHaveCount(0);
    expect(await page.getByRole("button", { name: "My library", exact: true }).boundingBox()).toEqual(tabs);
  } finally {
    held.release();
  }
});

test("practice recovers from a dropped request without showing an empty session", async ({ app, page }, testInfo) => {
  const prompt = "Recall the recovery answer";
  await app.createFact({
    prompt,
    answer: "Recovery answer",
    practices: [{ kind: "flashcard", prompt, answer: "Recovery answer" }],
  });
  const retry = gate();
  let requests = 0;

  await page.route(/\/api\/profiles\/1\/practice\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await route.abort("failed");
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    await startExtraPractice(page);
    await expect(page.getByRole("alert")).toContainText("Could not load practice.");
    await expect(page.getByRole("heading", { name: "No facts yet.", exact: true })).toBeHidden();
    await page.getByRole("button", { name: "Try again", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Loading practice…");
    await page.screenshot({ path: testInfo.outputPath("extra-practice-loading.png"), fullPage: true });
    retry.release();

    await expect(page.getByRole("heading", { name: prompt, exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Reveal answer", exact: true })).toBeEnabled();
    expect(requests).toBe(2);
  } finally {
    retry.release();
  }
});

test("matching retries an unavailable sample against the real library", async ({ app, page }) => {
  const facts = await Promise.all([1, 2, 3].map((number) => app.createFact({
    prompt: `Matching recovery question ${number}`,
    answer: `Matching recovery answer ${number}`,
    tags: ["synthetic-recovery-matching-bank"],
  })));
  const retry = gate();
  let requests = 0;

  await page.route(/\/api\/profiles\/1\/facts\/sample\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    await page.getByRole("button", { name: "Match pairs", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Could not load matching pairs.");
    await expect(page.getByRole("heading", { name: "Add three related facts with a shared topic tag to play." })).toBeHidden();
    await page.getByRole("button", { name: "Try again", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Loading matching pairs…");
    retry.release();

    const game = page.getByRole("region", { name: "Matching pairs game" });
    for (const fact of facts) {
      await expect(game.getByRole("button", { name: fact.prompt, exact: true })).toBeEnabled();
      await expect(game.getByRole("button", { name: fact.answer, exact: true })).toBeDisabled();
    }
    expect(requests).toBe(2);
  } finally {
    retry.release();
  }
});

test("a failed fact save preserves every draft field and retry inserts exactly once", async ({ app, page }) => {
  const retry = gate();
  let posts = 0;
  await page.route(/\/api\/profiles\/1\/facts$/, async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }

    posts += 1;
    if (posts === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    await page.getByRole("button", { name: "Add a fact", exact: true }).click();
    const form = page.getByRole("region", { name: "Add a fact", exact: true });
    await form.getByLabel("Question", { exact: true }).fill("What survives a failed save?");
    await form.getByLabel("Answer", { exact: true }).fill("Every draft field");
    await form.getByText("Context, source and tags (optional)", { exact: true }).click();
    await form.getByLabel("Context", { exact: true }).fill("A retry must preserve this context.");
    await form.getByLabel("Source", { exact: true }).fill("Household notebook");
    await form.getByLabel("Tags").fill("recovery, household");
    await form.getByRole("button", { name: "Save fact", exact: true }).click();

    await expect(page.getByRole("alert")).toContainText("Could not save the fact.");
    await expect(form.getByLabel("Question", { exact: true })).toHaveValue("What survives a failed save?");
    await expect(form.getByLabel("Answer", { exact: true })).toHaveValue("Every draft field");
    await expect(form.getByLabel("Context", { exact: true })).toHaveValue("A retry must preserve this context.");
    await expect(form.getByLabel("Source", { exact: true })).toHaveValue("Household notebook");
    await expect(form.getByLabel("Tags")).toHaveValue("recovery, household");
    expect((await (await app.request.get("/api/profiles/1/facts")).json()).total).toBe(0);

    await form.getByRole("button", { name: "Save fact", exact: true }).click();
    await expect(form.getByRole("button", { name: "Saving…", exact: true })).toBeDisabled();
    retry.release();

    await expect(page.getByRole("status").filter({ hasText: /^Fact saved\.$/ })).toBeVisible();
    await expect(form).toBeHidden();
    const saved = await (await app.request.get("/api/profiles/1/facts")).json();
    expect(saved.total).toBe(1);
    expect(saved.items[0]).toMatchObject({
      prompt: "What survives a failed save?",
      answer: "Every draft field",
      context: "A retry must preserve this context.",
      source: "Household notebook",
      tags: ["recovery", "household"],
    });
    expect(posts).toBe(2);
  } finally {
    retry.release();
  }
});

test("a failed review save lets practice finish and blocks leaving until one retry saves it", async ({ app, page }) => {
  const prompt = "Which review must survive a retry?";
  const fact = await app.createFact({
    prompt,
    answer: "This review",
    practices: [{ kind: "flashcard", prompt, answer: "This review" }],
  });
  const retry = gate();
  let posts = 0;
  await page.route(/\/api\/profiles\/1\/reviews$/, async (route) => {
    posts += 1;
    if (posts === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/app");
    await startExtraPractice(page);
    await page.getByLabel("Your answer").fill("My original attempt");
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toContainText("Could not save a review. Temporarily unavailable.");
    await expect(page.getByRole("status")).toHaveText("Saving…");
    await expect(page.getByText("1 review saved.", { exact: true })).toHaveCount(0);
    await expectNavigationDisabled(page);
    // The open menu can cover the alert on a phone, so close it before retrying.
    const profileMenu = page.getByLabel("Profile: Josiah", { exact: true });
    await profileMenu.click();
    await expect(page.getByRole("button", { name: "Profile settings", exact: true })).toBeDisabled();
    await profileMenu.click();
    expect((await (await app.request.get(`/api/profiles/1/facts/${fact.id}/reviews`)).json()).total).toBe(0);

    await page.getByRole("button", { name: "Retry save", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.getByRole("status")).toHaveText("Saving…");
    await profileMenu.click();
    await expect(page.getByRole("button", { name: "Profile settings", exact: true })).toBeDisabled();
    retry.release();

    await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Profile settings", exact: true })).toBeEnabled();
    const reviews = await (await app.request.get(`/api/profiles/1/facts/${fact.id}/reviews`)).json();
    expect(reviews.total).toBe(1);
    expect(reviews.items[0]).toMatchObject({
      fact_id: fact.id,
      practice_id: fact.practices[0].id,
      method: "flashcard",
      outcome: "good",
    });
    expect(posts).toBe(2);
  } finally {
    retry.release();
  }
});

test("a failed matching review keeps the round moving and later reviews wait their turn", async ({ app, page }) => {
  const facts = await Promise.all([1, 2, 3].map((number) => app.createFact({
    prompt: `Durable matching question ${number}`,
    answer: `Durable matching answer ${number}`,
    tags: ["synthetic-durable-matching-bank"],
  })));
  const retry = gate();
  const sent: number[] = [];
  await page.route(/\/api\/profiles\/1\/reviews$/, async (route) => {
    sent.push(route.request().postDataJSON().fact_id);
    if (sent.length === 1) {
      await unavailable(route);
      return;
    }

    if (sent.length === 2) {
      await retry.promise;
    }
    await route.continue();
  });

  try {
    await page.goto("/app");
    await page.getByRole("button", { name: "Match pairs", exact: true }).click();
    const game = page.getByRole("region", { name: "Matching pairs game" });
    await game.getByRole("button", { name: facts[0].prompt, exact: true }).click();
    await game.getByRole("button", { name: facts[0].answer, exact: true }).click();
    await game.getByRole("button", { name: "Hard Correct, with difficulty", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Could not save a review. Temporarily unavailable.");
    await expect(game.getByText("1 of 3 matched", { exact: true })).toBeVisible();
    await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toBeHidden();
    await expectNavigationDisabled(page);

    // The learner keeps playing; this rating queues behind the failed one.
    await game.getByRole("button", { name: facts[1].prompt, exact: true }).click();
    await game.getByRole("button", { name: facts[1].answer, exact: true }).click();
    await game.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(game.getByText("2 of 3 matched", { exact: true })).toBeVisible();
    expect(sent).toEqual([facts[0].id]);

    await page.getByRole("button", { name: "Retry save", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect.poll(() => sent).toEqual([facts[0].id, facts[0].id]);
    retry.release();

    await expect.poll(() => sent).toEqual([facts[0].id, facts[0].id, facts[1].id]);
    await expect(page.getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name: "Library", exact: true })).toBeEnabled();
    for (const [index, outcome] of [[0, "hard"], [1, "good"]] as const) {
      const fact = facts[index];
      await expect.poll(async () =>
        (await (await app.request.get(`/api/profiles/1/facts/${fact.id}/reviews`)).json()).items)
        .toMatchObject([{ fact_id: fact.id, practice_id: null, method: "matching", outcome }]);
    }
  } finally {
    retry.release();
  }
});

for (const staleResult of ["success", "failure"] as const) {
  test(`a stale search ${staleResult} cannot replace a newer search`, async ({ app, page }) => {
    const oldFact = await app.createFact({ prompt: "Old search apricot" });
    const newFact = await app.createFact({ prompt: "New search blueberry" });
    const oldStarted = gate();
    const oldRelease = gate();
    const oldFinished = gate();

    await page.route(/\/api\/profiles\/1\/facts\?/, async (route) => {
      const search = new URL(route.request().url()).searchParams.get("search");
      if (search !== "apricot") {
        await route.continue();
        return;
      }

      // Fetch genuine backend data before reordering its browser delivery.
      const response = staleResult === "success" ? await route.fetch() : undefined;
      oldStarted.release();
      await oldRelease.promise;
      try {
        if (response) {
          await route.fulfill({ response });
        } else {
          await unavailable(route);
        }
      } finally {
        oldFinished.release();
      }
    });

    try {
      await page.goto("/app");
      await expect(page.getByRole("heading", { name: oldFact.prompt, exact: true })).toBeVisible();
      const search = page.getByRole("searchbox", { name: "Search facts", exact: true });
      await search.fill("apricot");
      await oldStarted.promise;
      await expect(page.getByText("Loading library…", { exact: true })).toBeVisible();
      await search.fill("blueberry");
      await expect(page.getByRole("heading", { name: newFact.prompt, exact: true })).toBeVisible();

      const oldResponse = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/profiles/1/facts" &&
        new URL(response.url()).searchParams.get("search") === "apricot");
      oldRelease.release();
      await oldFinished.promise;
      await (await oldResponse).finished();
      // Cross the browser's microtask boundary after the old fetch resolves.
      await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));

      await expect(search).toHaveValue("blueberry");
      await expect(page.getByRole("heading", { name: newFact.prompt, exact: true })).toBeVisible();
      await expect(page.getByRole("heading", { name: oldFact.prompt, exact: true })).toBeHidden();
      await expect(page.getByRole("region", { name: "Saved facts" }).getByRole("article")).toHaveCount(1);
      await expect(page.getByRole("alert")).toBeHidden();
      await expect(page.getByText("Loading library…", { exact: true })).toBeHidden();
    } finally {
      oldRelease.release();
    }
  });
}
