import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import type { components } from "@keepsake/client";
import { test, expect, type TestApp } from "./fixtures";
import { startExtraPractice } from "./actions";

type StudyQueue = components["schemas"]["StudyQueue"];
type StudyQuestion = components["schemas"]["StudyQuestion"];
type StudySubmission = components["schemas"]["StudySubmission"];
type StudyResult = components["schemas"]["StudyResult"];
type Schedule = components["schemas"]["Schedule"];
type StudyEventPage = components["schemas"]["StudyEventPage"];
type SchedulerSettings = components["schemas"]["SchedulerSettings"];

async function schedule(app: TestApp, factId: number): Promise<Schedule> {
  const response = await app.request.get(`/api/study/facts/${factId}`);
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}

async function events(app: TestApp): Promise<StudyEventPage> {
  const response = await app.request.get("/api/study/events?after=0&limit=100");
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}

async function dueQueue(app: TestApp): Promise<StudyQueue> {
  const response = await app.request.get("/api/study/queue?limit=20");
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}

async function startScheduled(page: Page): Promise<StudyQueue> {
  const loaded = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/study/queue");
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  const response = await loaded;
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function attempt(page: Page, answer = "My unaided synthetic answer", waitForFeedback = true) {
  await page.getByRole("textbox", { name: "Your answer optional" }).fill(answer);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  if (waitForFeedback) {
    await expect(page.getByRole("heading", { name: "Rate your attempt", exact: true })).toBeVisible();
  }
}

async function rateGood(page: Page): Promise<StudyResult> {
  const saved = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/study/events" &&
    response.request().method() === "POST" &&
    response.request().postDataJSON().kind === "attempt");
  await page.getByRole("button", { name: /^Good\b/ }).click();
  const response = await saved;
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}

function submission(question: StudyQuestion, responseMs = 1_000): StudySubmission {
  return {
    submission_id: randomUUID(),
    target_id: question.target_id,
    practice_id: question.practice.id,
    content_revision: question.content_revision,
    expected_version: question.schedule_version,
    kind: "attempt",
    context: "scheduled",
    method: question.practice.kind,
    outcome: "good",
    response_ms: responseMs,
    attempted: true,
    assisted: false,
    prior_exposure: false,
  };
}

test("scheduled recall selects canonical prompts, previews intervals and persists a real recall", async ({ app, page }) => {
  const fact = await app.createFact({
    prompt: "Synthetic canonical recall question?",
    answer: "Synthetic canonical answer",
    practices: [
      { kind: "explain", prompt: "Explain the synthetic idea.", answer: "Synthetic explanation" },
      { kind: "apply", prompt: "Apply the synthetic idea.", answer: "Synthetic application" },
      { kind: "cloze", prompt: "The synthetic answer is ___ .", answer: "Synthetic canonical answer" },
    ],
  });
  const initial = await schedule(app, fact.id);
  await page.goto("/");
  const queue = await startScheduled(page);
  expect(queue.items).toHaveLength(1);
  expect(queue.items[0].practice).toMatchObject({ kind: "flashcard", prompt: fact.prompt });
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await expect(page.getByText("Scheduled recall", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Good\b/ })).toHaveCount(0);
  await attempt(page);
  await expect(page.getByRole("button", { name: /^Again\b/ })).toContainText("10 min");
  for (const name of ["Hard", "Good", "Easy"]) {
    await expect(page.getByRole("button", { name: new RegExp(`^${name}\\b`) })).toContainText(/\d+ days?/);
  }

  const result = await rateGood(page);
  expect(result.qualifying).toBe(true);
  expect(result.schedule.version).toBe(initial.version + 2);
  expect(result.schedule.stability).toBeGreaterThan(0);
  expect(result.schedule.due_at).toBeGreaterThan(result.schedule.last_review_at!);
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  await expect(page.getByText(/^Next review:/)).toBeVisible();
  expect((await dueQueue(app)).items).toEqual([]);
  const history = await app.request.get(`/api/facts/${fact.id}/reviews`);
  expect((await history.json()).items).toEqual([result.review]);
  await app.restart();
  expect(await schedule(app, fact.id)).toEqual(result.schedule);
  const durableEvents = await events(app);
  expect(durableEvents.items).toHaveLength(2);
  expect(durableEvents.items[0]).toMatchObject({ kind: "exposure", qualifying: false });
  expect(durableEvents.items[1]).toMatchObject({
    target_id: initial.target_id,
    kind: "attempt",
    context: "scheduled",
    qualifying: true,
    outcome: "good",
  });
});

test("a spoken recall attempt can be confirmed without typing", async ({ app, page }) => {
  await app.createFact();
  await page.goto("/");
  await startScheduled(page);
  await page.getByRole("checkbox", { name: "I tried to recall", exact: true }).check();
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  const result = await rateGood(page);
  expect(result.qualifying).toBe(true);
  expect(JSON.parse((await events(app)).items[0].request_json)).toMatchObject({
    attempted: true,
    assisted: false,
    prior_exposure: false,
  });
});

test("revealing without an attempt records exposure and offers no recall rating", async ({ app, page }) => {
  const fact = await app.createFact();
  const before = await schedule(app, fact.id);
  await page.goto("/");
  await startScheduled(page);
  const saved = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/study/events" && response.request().method() === "POST");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  const result: StudyResult = await (await saved).json();
  expect(result.qualifying).toBe(false);
  expect(result.review).toBeNull();
  await expect(page.getByText("Answer viewed without a recall attempt. This does not advance its schedule.", {
    exact: true,
  })).toBeVisible();
  await expect(page.getByRole("button", { name: /^(Again|Hard|Good|Easy)\b/ })).toHaveCount(0);
  expect(result.schedule).toMatchObject({
    stability: before.stability,
    difficulty: before.difficulty,
    due_at: before.due_at,
  });
  expect(result.schedule.introduced_at).not.toBeNull();
  expect(result.schedule.blocked_until).toBeGreaterThan(Math.floor(Date.now() / 1000));
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  expect((await dueQueue(app)).items).toEqual([]);
  expect((await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json()).total).toBe(0);
  await app.restart();
  expect(await schedule(app, fact.id)).toEqual(result.schedule);
});

test("feedback is recorded before rating so reloading cannot turn a viewed answer into unaided recall", async ({ app, page }) => {
  const fact = await app.createFact();
  await page.goto("/");
  await startScheduled(page);
  await attempt(page);
  await expect(page.getByRole("button", { name: /^Good\b/ })).toBeEnabled();
  const feedback = (await events(app)).items;
  expect(feedback).toHaveLength(1);
  expect(feedback[0]).toMatchObject({ kind: "exposure", qualifying: false });
  expect(JSON.parse(feedback[0].request_json)).toMatchObject({ attempted: true, outcome: null });
  await page.reload();
  await startScheduled(page);
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  expect((await schedule(app, fact.id)).stability).toBeNull();
  expect((await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json()).total).toBe(0);
});

test("a lost feedback response blocks ratings until the same exposure is confirmed", async ({ app, page }, testInfo) => {
  const fact = await app.createFact();
  const requests: StudySubmission[] = [];
  await page.route(/\/api\/study\/events$/, async (route) => {
    const body = route.request().postDataJSON() as StudySubmission;
    if (body.kind !== "exposure") {
      await route.continue();
      return;
    }
    requests.push(body);
    if (requests.length === 1) {
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBeTruthy();
      await route.abort("failed");
      return;
    }
    await route.continue();
  });
  await page.goto("/");
  await startScheduled(page);
  await attempt(page, "My unaided synthetic answer", false);
  await expect(page.getByRole("alert")).toContainText("Could not save this activity.");
  await expect(page.getByRole("button", { name: /^Good\b/ })).toHaveCount(0);
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("scheduled-feedback-failure.png"), fullPage: true });
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByRole("button", { name: /^Good\b/ })).toBeEnabled();
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual(requests[0]);
  const feedback = await schedule(app, fact.id);
  const result = await rateGood(page);
  expect(result.qualifying).toBe(true);
  expect(result.schedule.version).toBe(feedback.version + 1);
  const history = (await events(app)).items;
  expect(history).toHaveLength(2);
  expect(history[0].response_ms).toBe(history[1].response_ms);
  expect(JSON.parse(history[1].request_json).feedback_event_id).toBe(history[0].id);
});

test("unattempted answer exposures consume the bounded daily new-card intake", async ({ app, page }) => {
  for (let index = 0; index < 7; index += 1) {
    await app.createFact({ prompt: `Synthetic new intake question ${index}?` });
  }
  await page.goto("/");
  const queue = await startScheduled(page);
  expect(queue.items).toHaveLength(5);
  for (const question of queue.items) {
    await expect(page.getByRole("heading", { name: question.practice.prompt, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: "Continue", exact: true }).click();
  }
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  const remaining = await dueQueue(app);
  expect(remaining.items).toEqual([]);
  expect(remaining.new_remaining).toBe(0);
  const recorded = (await events(app)).items;
  expect(recorded).toHaveLength(5);
  expect(recorded.every((event) => event.kind === "exposure" && !event.qualifying)).toBe(true);
});

test("retrying after a committed but lost review response keeps one UUID and one durable rating", async ({ app, page }) => {
  const fact = await app.createFact();
  const requests: StudySubmission[] = [];
  let committed: StudyResult | undefined;
  await page.route(/\/api\/study\/events$/, async (route) => {
    const body = route.request().postDataJSON() as StudySubmission;
    if (body.kind !== "attempt") {
      await route.continue();
      return;
    }
    requests.push(body);
    if (requests.length === 1) {
      const response = await route.fetch();
      expect(response.ok(), await response.text()).toBeTruthy();
      committed = await response.json();
      await route.abort("failed");
      return;
    }
    await route.continue();
  });
  await page.goto("/");
  await startScheduled(page);
  await attempt(page);
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await expect(page.getByRole("alert")).toContainText("Could not save this attempt.");
  await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Easy\b/ })).toBeDisabled();
  const retried = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/study/events" && response.request().method() === "POST");
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  expect(await (await retried).json()).toEqual(committed);
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual(requests[0]);
  expect((await events(app)).items).toHaveLength(2);
  expect((await (await app.request.get(`/api/facts/${fact.id}/reviews`)).json()).total).toBe(1);
  await app.restart();
  expect((await events(app)).items).toHaveLength(2);
  expect(await schedule(app, fact.id)).toEqual(committed!.schedule);
});

test("a stale browser tab cannot overwrite a newer schedule and can refresh the due queue", async ({ app, page, context }) => {
  const fact = await app.createFact();
  const other = await context.newPage();
  await Promise.all([page.goto("/"), other.goto(app.url)]);
  const [first, second] = await Promise.all([startScheduled(page), startScheduled(other)]);
  expect(first.items[0].schedule_version).toBe(second.items[0].schedule_version);
  await attempt(page);
  const accepted = await rateGood(page);
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();

  const rejected = other.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/study/events" && response.request().method() === "POST");
  await attempt(other, "My unaided synthetic answer", false);
  expect((await rejected).status()).toBe(409);
  await expect(other.getByRole("alert")).toHaveText("This question changed in another tab. Refresh practice.");
  await other.getByRole("button", { name: "Refresh practice", exact: true }).click();
  await expect(other.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  expect(await schedule(app, fact.id)).toEqual(accepted.schedule);
  expect((await events(app)).items).toHaveLength(2);
});

test("library exposure retries a lost committed response and survives restart without changing memory", async ({ app, page }) => {
  const fact = await app.createFact();
  const before = await schedule(app, fact.id);
  const bodies: unknown[] = [];
  await page.route(/\/api\/study\/exposures$/, async (route) => {
    bodies.push(route.request().postDataJSON());
    if (bodies.length === 1) {
      const response = await route.fetch();
      expect(response.ok()).toBeTruthy();
      await route.abort("failed");
      return;
    }
    await route.continue();
  });
  await page.goto("/");
  await page.getByRole("heading", { name: fact.prompt, exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Could not save this activity.");
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByText("Saving…", { exact: true })).toHaveCount(0);
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  expect(bodies).toHaveLength(2);
  expect(bodies[1]).toEqual(bodies[0]);
  const after = await schedule(app, fact.id);
  expect(after).toMatchObject({ stability: before.stability, difficulty: before.difficulty, due_at: before.due_at });
  expect(after.version).toBe(before.version + 1);
  expect(after.blocked_until).toBeGreaterThan(Math.floor(Date.now() / 1000));
  expect((await events(app)).items).toHaveLength(1);
  await startScheduled(page);
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  await app.restart();
  expect(await schedule(app, fact.id)).toEqual(after);
});

test("reopening a library fact hides its cached answer until the new exposure is saved", async ({ app, page }) => {
  const fact = await app.createFact();
  await page.goto("/");
  const summary = page.getByRole("heading", { name: fact.prompt, exact: true });
  await summary.click();
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  await summary.click();
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);

  const bodies: unknown[] = [];
  await page.route(/\/api\/study\/exposures$/, async (route) => {
    bodies.push(route.request().postDataJSON());
    if (bodies.length === 1) {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: "Synthetic reopen failure" }) });
      return;
    }
    await route.continue();
  });
  await summary.click();
  await expect(page.getByRole("alert")).toContainText("Could not save this activity.");
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  expect(bodies).toHaveLength(2);
  expect(bodies[1]).toEqual(bodies[0]);
});

test("multiple-choice options record recognition exposure before any answer is checked", async ({ app, page }) => {
  const fact = await app.createFact({
    prompt: "Synthetic canonical option question?",
    answer: "Synthetic correct option",
    practices: [{
      kind: "multiple_choice",
      prompt: "Which synthetic option is correct?",
      answer: "Synthetic correct option",
      choices: ["Synthetic correct option", "Synthetic wrong option"],
    }],
  });
  await page.goto("/");
  const loaded = page.waitForResponse((response) => new URL(response.url()).pathname === "/api/practice");
  await startExtraPractice(page);
  const extra: components["schemas"]["PracticeSession"] = await (await loaded).json();
  if (extra.items[0].practice.kind === "flashcard") {
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: /^Good\b/ }).click();
  }

  const stage = page.getByRole("region", { name: "Multiple choice", exact: true });
  await expect(stage.getByRole("radio", { name: fact.answer, exact: true })).toBeVisible();
  await expect(stage.getByRole("button", { name: "Check answer", exact: true })).toBeDisabled();
  const recognition = (await events(app)).items.filter((event) =>
    event.method === "multiple_choice" && event.kind === "exposure");
  expect(recognition).toHaveLength(1);
  expect(recognition[0].qualifying).toBe(false);
  await page.getByRole("button", { name: "Scheduled recall", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();
  expect((await schedule(app, fact.id)).stability).toBeNull();
});

test("matching exposure failure blocks play until its batch is saved and never advances recall", async ({ app, page }, testInfo) => {
  const facts = [];
  for (let index = 0; index < 3; index += 1) {
    facts.push(await app.createFact({
      prompt: `Synthetic exposure pair ${index}?`,
      answer: `Synthetic exposure answer ${index}`,
      tags: ["synthetic-exposure-bank"],
    }));
  }
  const bodies: unknown[] = [];
  await page.route(/\/api\/study\/exposures$/, async (route) => {
    bodies.push(route.request().postDataJSON());
    if (bodies.length === 1) {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: "Synthetic failure" }) });
      return;
    }
    await route.continue();
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Match pairs", exact: true }).click();
  const game = page.getByRole("region", { name: "Matching pairs game" });
  await expect(page.getByRole("alert")).toContainText("Could not save this activity.");
  await expect(game).toHaveCount(0);
  for (const fact of facts) {
    await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
  }
  await page.screenshot({ path: testInfo.outputPath("matching-exposure-failure.png"), fullPage: true });
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByText("Saving…", { exact: true })).toHaveCount(0);
  for (const fact of facts) {
    await expect(game.getByRole("button", { name: fact.prompt, exact: true })).toBeEnabled();
    const target = await schedule(app, fact.id);
    expect(target.stability).toBeNull();
    expect(target.due_at).toBeNull();
    expect(target.blocked_until).toBeGreaterThan(Math.floor(Date.now() / 1000));
  }
  await page.screenshot({ path: testInfo.outputPath("matching-ready.png"), fullPage: true });
  expect(bodies[1]).toEqual(bodies[0]);
  expect((await events(app)).items).toHaveLength(3);
  expect((await dueQueue(app)).items).toEqual([]);
});

test("the daily study budget is a notice and does not hide available practice", async ({ app, page }) => {
  const first = await app.createFact({ prompt: "Synthetic first budget question?" });
  const second = await app.createFact({ prompt: "Synthetic second budget question?" });
  const settingsResponse = await app.request.get("/api/study/settings");
  const settings: SchedulerSettings = await settingsResponse.json();
  const update = await app.request.put("/api/study/settings", {
    data: { timezone: "Europe/London", new_limit: 5, daily_budget_seconds: 60, expected_version: settings.version },
  });
  expect(update.ok(), await update.text()).toBeTruthy();
  const queue = await dueQueue(app);
  const question = queue.items.find((item) => item.fact.id === first.id)!;
  const saved = await app.request.post("/api/study/events", { data: submission(question, 60_000) });
  expect(saved.ok(), await saved.text()).toBeTruthy();
  await page.goto("/");
  const remaining = await startScheduled(page);
  expect(remaining.budget_reached).toBe(true);
  expect(remaining.items.map((item) => item.fact.id)).toContain(second.id);
  await expect(page.getByRole("status").filter({ hasText: "Daily study budget reached." })).toBeVisible();
  await expect(page.getByRole("heading", { name: second.prompt, exact: true })).toBeVisible();
  await attempt(page);
  expect((await rateGood(page)).qualifying).toBe(true);
});
