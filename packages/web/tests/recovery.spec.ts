import type { Page, Route } from "@playwright/test";
import { test, expect } from "./fixtures";

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
  await expect(page.getByRole("button", { name: "Add a fact", exact: true })).toBeDisabled();
  await expect(page.locator(".ratings button")).toHaveCount(4);
  for (const rating of await page.locator(".ratings button").all()) {
    await expect(rating).toBeDisabled();
  }
}

test("library retries a failed load and shows loading until the real response arrives", async ({ app, page }) => {
  const fact = await app.createFact({ prompt: "Recovery library question" });
  const retry = gate();
  let requests = 0;

  await page.route(/\/api\/facts\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/");
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

test("practice recovers from a dropped request without showing an empty session", async ({ app, page }) => {
  const prompt = "Recall the recovery answer";
  await app.createFact({
    prompt,
    answer: "Recovery answer",
    practices: [{ kind: "flashcard", prompt, answer: "Recovery answer" }],
  });
  const retry = gate();
  let requests = 0;

  await page.route(/\/api\/practice\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await route.abort("failed");
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/");
    await page.getByRole("button", { name: "Practise", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Could not load practice.");
    await expect(page.getByRole("heading", { name: "No facts yet.", exact: true })).toBeHidden();
    await page.getByRole("button", { name: "Try again", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Loading practice…");
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

  await page.route(/\/api\/facts\/sample\?/, async (route) => {
    requests += 1;
    if (requests === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/");
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
  await page.route(/\/api\/facts$/, async (route) => {
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
    await page.goto("/");
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
    expect((await (await app.request.get("/api/facts")).json()).total).toBe(0);

    await form.getByRole("button", { name: "Save fact", exact: true }).click();
    await expect(form.getByRole("button", { name: "Saving…", exact: true })).toBeDisabled();
    retry.release();

    await expect(page.getByRole("status").filter({ hasText: /^Fact saved\.$/ })).toBeVisible();
    await expect(form).toBeHidden();
    const saved = await (await app.request.get("/api/facts")).json();
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

test("review retry keeps the revealed question and blocks navigation until one review is saved", async ({ app, page }) => {
  const prompt = "Which question must remain during a retry?";
  const fact = await app.createFact({
    prompt,
    answer: "This question",
    practices: [{ kind: "flashcard", prompt, answer: "This question" }],
  });
  const retry = gate();
  let posts = 0;
  await page.route(/\/api\/reviews$/, async (route) => {
    posts += 1;
    if (posts === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/");
    await page.getByRole("button", { name: "Practise", exact: true }).click();
    await page.getByLabel("Your answer").fill("My original attempt");
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Review was not saved.");
    await expect(page.getByRole("heading", { name: prompt, exact: true })).toBeVisible();
    await expect(page.getByLabel("Your answer")).toHaveValue("My original attempt");
    await expect(page.getByLabel("Your answer")).toHaveAttribute("readonly", "");
    expect((await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json()).total).toBe(0);

    await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(page.getByRole("status")).toHaveText("Saving review…");
    await expectNavigationDisabled(page);
    await expect(page.getByRole("heading", { name: prompt, exact: true })).toBeVisible();
    retry.release();

    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
    const reviews = await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json();
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

test("matching review retry preserves the pair and only counts a durable review", async ({ app, page }) => {
  const facts = await Promise.all([1, 2, 3].map((number) => app.createFact({
    prompt: `Durable matching question ${number}`,
    answer: `Durable matching answer ${number}`,
    tags: ["synthetic-durable-matching-bank"],
  })));
  const fact = facts[0];
  const retry = gate();
  let posts = 0;
  await page.route(/\/api\/reviews$/, async (route) => {
    posts += 1;
    if (posts === 1) {
      await unavailable(route);
      return;
    }

    await retry.promise;
    await route.continue();
  });

  try {
    await page.goto("/");
    await page.getByRole("button", { name: "Match pairs", exact: true }).click();
    const game = page.getByRole("region", { name: "Matching pairs game" });
    await game.getByRole("button", { name: fact.prompt, exact: true }).click();
    await game.getByRole("button", { name: fact.answer, exact: true }).click();
    await game.getByRole("button", { name: "Hard Correct, with difficulty", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Review was not saved.");
    await expect(game.getByText("0 of 3 matched", { exact: true })).toBeVisible();
    await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toBeVisible();
    expect((await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json()).total).toBe(0);

    await game.getByRole("button", { name: "Hard Correct, with difficulty", exact: true }).click();
    await expect(page.getByText("Saving review…", { exact: true })).toBeVisible();
    await expectNavigationDisabled(page);
    retry.release();

    await expect(game.getByText("1 of 3 matched", { exact: true })).toBeVisible();
    await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toBeHidden();
    await expect(game.getByRole("button", { name: new RegExp(fact.prompt) })).toBeDisabled();
    const reviews = await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json();
    expect(reviews.total).toBe(1);
    expect(reviews.items[0]).toMatchObject({
      fact_id: fact.id,
      practice_id: null,
      method: "matching",
      outcome: "hard",
    });
    expect(posts).toBe(2);
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

    await page.route(/\/api\/facts\?/, async (route) => {
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
      await page.goto("/");
      await expect(page.getByRole("heading", { name: oldFact.prompt, exact: true })).toBeVisible();
      const search = page.getByRole("searchbox", { name: "Search facts", exact: true });
      await search.fill("apricot");
      await oldStarted.promise;
      await expect(page.getByText("Loading library…", { exact: true })).toBeVisible();
      await search.fill("blueberry");
      await expect(page.getByRole("heading", { name: newFact.prompt, exact: true })).toBeVisible();

      const oldResponse = page.waitForResponse((response) =>
        new URL(response.url()).pathname === "/api/facts" &&
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
