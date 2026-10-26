import type { Page } from "@playwright/test";
import type { components } from "@keepsake/client";
import { test, expect, type TestApp } from "./fixtures";
import { startExtraPractice } from "./actions";

type StudySubmission = components["schemas"]["StudySubmission"];
type StudyEventPage = components["schemas"]["StudyEventPage"];
type SchedulerSettings = components["schemas"]["SchedulerSettings"];

function gate() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });

  return { promise, release };
}

function countRequests(page: Page, pathname: string) {
  const count = { value: 0 };
  page.on("request", (request) => {
    if (request.method() === "GET" && new URL(request.url()).pathname === pathname) {
      count.value += 1;
    }
  });
  return count;
}

async function events(app: TestApp): Promise<StudyEventPage> {
  const response = await app.request.get("/api/profiles/1/study/events?after=0&limit=100");
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}

async function recallAndReveal(page: Page, answer: string) {
  await page.getByRole("textbox", { name: "Your answer optional" }).fill(answer);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
}

test("a scheduled rating saves in the background while later saves keep their order", async ({ app, page }) => {
  const first = await app.createFact({ prompt: "Synthetic first pipelined question?", answer: "First pipelined answer" });
  const second = await app.createFact({ prompt: "Synthetic second pipelined question?", answer: "Second pipelined answer" });
  const held = gate();
  const sent: StudySubmission[] = [];
  await page.route(/\/api\/profiles\/1\/study\/events$/, async (route) => {
    const body = route.request().postDataJSON() as StudySubmission;
    sent.push(body);
    if (body.kind === "attempt" && body.target_id === sent[0].target_id) {
      await held.promise;
    }
    await route.continue();
  });

  try {
    await page.goto("/p/1/practice");
    await expect(page.getByRole("heading", { name: first.prompt, exact: true })).toBeVisible();
    await recallAndReveal(page, "My first attempt");
    await page.getByRole("button", { name: /^Good\b/ }).click();

    // The next question is ready while the first rating is still on its way.
    await expect(page.getByRole("heading", { name: second.prompt, exact: true })).toBeVisible();
    await expect.poll(() => sent.map((body) => body.kind)).toEqual(["exposure", "attempt"]);
    await recallAndReveal(page, "My second attempt");
    // Feedback waits behind the earlier save, so the answer stays hidden until the server has both.
    await expect(page.getByRole("status")).toHaveText("Saving…");
    await expect(page.getByText(second.answer, { exact: true })).toHaveCount(0);
    expect(sent).toHaveLength(2);
    held.release();

    await expect(page.getByText(second.answer, { exact: true })).toBeVisible();
    await page.getByRole("button", { name: /^Good\b/ }).click();
    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByText("2 reviews saved.", { exact: true })).toBeVisible();
    await expect(page.getByText(/^Next review:/)).toBeVisible();

    const recorded = (await events(app)).items;
    expect(recorded.map((event) => [event.kind, event.target_id])).toEqual(
      sent.map((body) => [body.kind, body.target_id]),
    );
    expect(recorded.map((event) => event.kind)).toEqual(["exposure", "attempt", "exposure", "attempt"]);
    expect(recorded.filter((event) => event.kind === "attempt").every((event) => event.qualifying)).toBe(true);
  } finally {
    held.release();
  }
});

test("the summary waits for the last save and Continue practice starts without another request", async ({ app, page }) => {
  // An allowance of 21 new facts leaves one for the session after a full queue of 20.
  const settings: SchedulerSettings = await (await app.request.get("/api/profiles/1/study/settings")).json();
  const update = await app.request.put("/api/profiles/1/study/settings", {
    data: { timezone: settings.timezone, new_limit: 21, daily_budget_seconds: 900, expected_version: settings.version },
  });
  expect(update.ok(), await update.text()).toBeTruthy();
  const facts = [];
  for (let index = 0; index < 21; index += 1) {
    facts.push(await app.createFact({ prompt: `Synthetic continuation question ${index}?` }));
  }
  const queueRequests = countRequests(page, "/api/profiles/1/study/queue");
  const held = gate();
  let attempts = 0;
  await page.route(/\/api\/profiles\/1\/study\/events$/, async (route) => {
    if (route.request().postDataJSON().kind === "attempt" && ++attempts === 20) {
      await held.promise;
    }
    await route.continue();
  });

  try {
    await page.goto("/p/1/practice");
    for (let index = 0; index < 20; index += 1) {
      await expect(page.getByText(`Flashcards · ${index + 1} of 20`, { exact: true })).toBeVisible();
      await page.getByLabel("I tried to recall", { exact: true }).check();
      await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
      await page.getByRole("button", { name: /^Again\b/ }).click();
    }

    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByRole("status")).toHaveText("Saving…");
    await expect(page.getByRole("button", { name: "Continue practice", exact: true })).toHaveCount(0);
    expect(queueRequests.value).toBe(1);
    held.release();

    await expect(page.getByText("20 reviews saved.", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Continue practice", exact: true }).click();
    await expect(page.getByRole("heading", { name: facts[20].prompt, exact: true })).toBeVisible();
    await expect(page.getByText("Flashcards · 1 of 1", { exact: true })).toBeVisible();
    expect(queueRequests.value).toBe(2);
  } finally {
    held.release();
  }
});

test("extra practice loads its next session while the learner finishes the current one", async ({ app, page }) => {
  for (const number of [1, 2]) {
    await app.createFact({ prompt: `Synthetic extra round question ${number}?` });
  }
  const samples = countRequests(page, "/api/profiles/1/practice");
  await page.goto("/");
  await startExtraPractice(page);

  for (const number of [1, 2]) {
    await expect(page.getByText(`Flashcards · ${number} of 2`, { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: /^Good\b/ }).click();
  }
  await expect(page.getByText("2 reviews saved.", { exact: true })).toBeVisible();
  expect(samples.value).toBe(2);

  await page.getByRole("button", { name: "Practise again", exact: true }).click();
  await expect(page.getByText("Flashcards · 1 of 2", { exact: true })).toBeVisible();
  expect(samples.value).toBe(2);
});

test("matching loads the next round while the learner rates the last pair", async ({ app, page }) => {
  const facts = [];
  for (const number of [1, 2, 3]) {
    facts.push(await app.createFact({
      prompt: `Synthetic prefetched pair ${number}?`,
      answer: `Synthetic prefetched answer ${number}`,
      tags: ["synthetic-prefetch-bank"],
    }));
  }
  const samples = countRequests(page, "/api/profiles/1/facts/sample");
  await page.goto("/");
  await page.getByRole("button", { name: "Match pairs", exact: true }).click();
  const game = page.getByRole("region", { name: "Matching pairs game" });

  for (const [index, fact] of facts.entries()) {
    await game.getByRole("button", { name: fact.prompt, exact: true }).click();
    await game.getByRole("button", { name: fact.answer, exact: true }).click();
    await expect.poll(() => samples.value).toBe(index === facts.length - 1 ? 2 : 1);
    await game.getByRole("button", { name: /^Good\b/ }).click();
  }
  await expect(game.getByRole("status")).toHaveText("Round complete.");
  await game.getByRole("button", { name: "Play again", exact: true }).click();
  await expect(game.getByText("0 of 3 matched", { exact: true })).toBeVisible();
  await expect(game.getByRole("button", { name: facts[0].prompt, exact: true })).toBeEnabled();
  expect(samples.value).toBe(2);
});

test("opening a practice address requests the due queue alongside the profiles", async ({ app, page }) => {
  const fact = await app.createFact();
  const queueRequests = countRequests(page, "/api/profiles/1/study/queue");
  const profiles = gate();
  await page.route(/\/api\/profiles$/, async (route) => {
    await profiles.promise;
    await route.continue();
  });

  try {
    await page.goto("/p/1/practice");
    await expect.poll(() => queueRequests.value).toBe(1);
    profiles.release();
    await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
    expect(queueRequests.value).toBe(1);
  } finally {
    profiles.release();
  }
});

test("a refused rating is reported without stopping the session", async ({ app, page }) => {
  const first = await app.createFact({ prompt: "Synthetic refused rating question?" });
  const second = await app.createFact({ prompt: "Synthetic accepted rating question?" });
  let refused: number | undefined;
  await page.route(/\/api\/profiles\/1\/study\/events$/, async (route) => {
    const body = route.request().postDataJSON() as StudySubmission;
    refused ??= body.target_id;
    if (body.kind === "attempt" && body.target_id === refused) {
      // Deliberately refuse the first rating as an expired attempt would be.
      await route.fulfill({
        status: 409,
        contentType: "application/json",
        body: JSON.stringify({ error: "This attempt expired. Refresh practice." }),
      });
      return;
    }
    await route.continue();
  });

  await page.goto("/p/1/practice");
  await expect(page.getByRole("heading", { name: first.prompt, exact: true })).toBeVisible();
  await recallAndReveal(page, "An attempt the server refuses");
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await expect(page.getByRole("alert")).toHaveText("A review was not saved. This attempt expired. Refresh practice.");
  await expect(page.getByRole("heading", { name: second.prompt, exact: true })).toBeVisible();

  await recallAndReveal(page, "An attempt the server accepts");
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  const attempts = (await events(app)).items.filter((event) => event.kind === "attempt");
  expect(attempts).toHaveLength(1);
  expect(attempts[0].target_id).not.toBe(refused);
  expect(attempts[0].qualifying).toBe(true);
});
