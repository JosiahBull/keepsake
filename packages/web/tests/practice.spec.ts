import type { APIRequestContext, Page } from "@playwright/test";
import type { components } from "@keepsake/client";
import { test, expect, type Fact, type NewFact } from "./fixtures";

type PracticeKind = components["schemas"]["PracticeKind"];
type PracticeQuestion = components["schemas"]["PracticeQuestion"];
type PracticeSession = components["schemas"]["PracticeSession"];
type Review = components["schemas"]["Review"];
type ReviewPage = components["schemas"]["ReviewPage"];
type Outcome = components["schemas"]["ReviewOutcome"];
type RunningApp = { url: string; request: APIRequestContext };

const ratings: Record<Outcome, RegExp> = {
  again: /^Again\b/,
  hard: /^Hard\b/,
  good: /^Good\b/,
  easy: /^Easy\b/,
};

const practiceNames: Record<PracticeKind, string> = {
  flashcard: "Flashcards",
  multiple_choice: "Multiple choice",
  cloze: "Fill the gap",
  explain: "Explain it",
  apply: "Apply it",
};

function practice(kind: PracticeKind): NonNullable<NewFact["practices"]>[number] {
  return {
    kind,
    prompt: kind === "cloze" ? "The test satellite orbits ___ ." : `Synthetic ${kind} question?`,
    answer: "The fictional planet Luma",
    choices: kind === "multiple_choice"
      ? ["The fictional planet Luma", "The fictional planet Neri", "The fictional planet Sora"]
      : [],
    explanation: `Synthetic ${kind} explanation for this test only.`,
  };
}

async function reviewHistory(app: RunningApp, factId: number): Promise<ReviewPage> {
  const response = await app.request.get(`${app.url}/api/facts/${factId}/reviews`);
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function freezeClock(page: Page) {
  await page.clock.install({ time: new Date("2026-10-24T12:00:00Z") });
  await page.clock.pauseAt(new Date("2026-10-24T12:01:00Z"));
}

async function beginPractice(page: Page): Promise<PracticeSession> {
  const response = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/practice" && response.request().method() === "GET",
  );
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
  const loaded = await response;
  expect(loaded.ok()).toBeTruthy();
  expect(new URL(loaded.url()).searchParams.get("limit")).toBe("20");
  return loaded.json();
}

async function revealQuestion(page: Page, question: PracticeQuestion, correctChoice = true) {
  const stage = page.getByRole("region", { name: practiceNames[question.practice.kind], exact: true });
  await expect(stage.getByRole("heading", { name: question.practice.prompt, exact: true })).toBeVisible();
  await expect(stage.getByRole("heading", { name: "Rate your attempt", exact: true })).toHaveCount(0);
  for (const rating of Object.values(ratings)) {
    await expect(stage.getByRole("button", { name: rating })).toHaveCount(0);
  }

  // Browser time advances without delaying the suite or changing the Rust server's clock.
  await page.clock.fastForward(1_500);

  if (question.practice.kind === "multiple_choice") {
    const check = stage.getByRole("button", { name: "Check answer", exact: true });
    await expect(check).toBeDisabled();
    await expect(stage.getByRole("radio")).toHaveCount(question.practice.choices.length);
    const selected = correctChoice
      ? question.practice.answer
      : question.practice.choices.find((choice) => choice !== question.practice.answer)!;
    await stage.getByRole("radio", { name: selected, exact: true }).check();
    await expect(check).toBeEnabled();
    await check.click();
    await expect(stage.getByRole("heading", {
      name: correctChoice ? "Correct" : "Correct answer",
      exact: true,
    })).toBeVisible();
    for (const option of question.practice.choices) {
      await expect(stage.getByRole("radio", { name: option, exact: true })).toBeDisabled();
    }
  } else {
    const response = stage.getByRole("textbox", { name: "Your answer optional" });
    await response.fill("My synthetic recall attempt.");
    await stage.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await expect(response).toHaveValue("My synthetic recall attempt.");
    await expect(response).toHaveJSProperty("readOnly", true);
    await expect(stage.getByRole("heading", { name: "Answer", exact: true })).toBeVisible();
  }

  await expect(stage.getByRole("heading", { name: "Rate your attempt", exact: true })).toBeVisible();
  if (question.practice.explanation) {
    await expect(stage.getByText(question.practice.explanation, { exact: true })).toBeVisible();
  }
  if (question.fact.source) {
    await expect(stage.getByRole("link", { name: question.fact.source, exact: true }))
      .toHaveAttribute("href", question.fact.source);
  }

  // The recorded latency belongs to the recall attempt, not time spent reading feedback.
  await page.clock.fastForward(7_000);
}

async function rate(page: Page, outcome: Outcome): Promise<Review> {
  const response = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/reviews" && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: ratings[outcome] }).click();
  const saved = await response;
  expect(saved.ok()).toBeTruthy();
  const review: Review = await saved.json();
  expect(review.outcome).toBe(outcome);
  expect(Number.isInteger(review.response_ms)).toBeTruthy();
  expect(review.response_ms).toBe(1_500);
  expect(review.reviewed_at).toBeTruthy();
  return review;
}

const cases: { kind: PracticeKind; outcome: Outcome; correctChoice?: boolean; title: string }[] = [
  { kind: "flashcard", outcome: "again", title: "flashcard recall and Again" },
  { kind: "cloze", outcome: "hard", title: "fill-the-gap recall and Hard" },
  { kind: "explain", outcome: "good", title: "explanation recall and Good" },
  { kind: "apply", outcome: "easy", title: "application recall and Easy" },
  { kind: "multiple_choice", outcome: "again", correctChoice: false, title: "incorrect multiple choice" },
  { kind: "multiple_choice", outcome: "easy", correctChoice: true, title: "correct multiple choice" },
];

for (const scenario of cases) {
  test(`${scenario.title} saves durable reviews and completes the session`, async ({ app, page }) => {
    const prompt = practice(scenario.kind);
    const fact = await app.createFact({
      // Match the explicit flashcard to avoid creating a duplicate default prompt.
      prompt: scenario.kind === "flashcard" ? prompt.prompt : "Synthetic satellite fact?",
      answer: prompt.answer,
      context: "Synthetic background context.",
      source: "https://example.invalid/synthetic-learning-source",
      practices: [prompt],
    });
    await freezeClock(page);
    await page.goto("/");
    const session = await beginPractice(page);
    expect(session.items).toHaveLength(scenario.kind === "flashcard" ? 1 : 2);
    expect(session.items.some((item) => item.practice.kind === scenario.kind)).toBeTruthy();
    await expect(page.getByRole("combobox")).toHaveCount(0);

    const saved: Review[] = [];
    for (const question of session.items) {
      await revealQuestion(page, question, scenario.correctChoice ?? true);
      const review = await rate(page, scenario.outcome);
      expect(review).toMatchObject({
        fact_id: fact.id,
        practice_id: question.practice.id,
        method: question.practice.kind,
      });
      saved.push(review);
    }

    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByText(`${saved.length} ${saved.length === 1 ? "review" : "reviews"} saved.`, {
      exact: true,
    })).toBeVisible();
    expect((await reviewHistory(app, fact.id)).total).toBe(saved.length);

    await app.restart();
    const durable = await reviewHistory(app, fact.id);
    expect(durable.total).toBe(saved.length);
    // Review history uses ascending IDs so after_id can continue chronologically.
    expect(durable.items).toEqual(saved);
    expect(durable.next_cursor).toBeNull();
  });
}

test("Practise automatically mixes every supported format without a mode selector", async ({ app, page }) => {
  const kinds: PracticeKind[] = ["flashcard", "multiple_choice", "cloze", "explain", "apply"];
  const prompts = kinds.map(practice);
  const fact = await app.createFact({
    prompt: prompts[0].prompt,
    answer: prompts[0].answer,
    practices: prompts,
  });
  await freezeClock(page);
  await page.goto("/");
  const session = await beginPractice(page);
  expect(session.items.map((item) => item.practice.kind).sort()).toEqual([...kinds].sort());
  await expect(page.getByRole("combobox")).toHaveCount(0);

  for (const question of session.items) {
    await revealQuestion(page, question);
    await rate(page, "good");
  }

  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  const history = await reviewHistory(app, fact.id);
  expect(history.total).toBe(5);
  expect(history.items.map((review) => review.method).sort()).toEqual([...kinds].sort());
  await page.getByRole("button", { name: "Practise again", exact: true }).click();
  await expect(page.getByRole("button", { name: "Reveal answer", exact: true })
    .or(page.getByRole("button", { name: "Check answer", exact: true }))).toBeVisible();
  expect((await reviewHistory(app, fact.id)).total).toBe(5);
});

test("practice samples the whole library independently of library page and search", async ({ app, page }) => {
  const facts: Fact[] = [];
  for (let index = 0; index < 26; index += 1) {
    facts.push(await app.createFact({
      prompt: `Synthetic household item ${index}?`,
      answer: `Synthetic household answer ${index}.`,
    }));
  }
  await page.goto("/");
  await page.getByRole("navigation", { name: "Library pages" })
    .getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.getByRole("heading", { name: facts[0].prompt, exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "Saved facts" }).getByRole("article")).toHaveCount(1);
  await page.getByRole("searchbox", { name: "Search facts" }).fill("nonexistent synthetic search");
  await expect(page.getByText("No facts match “nonexistent synthetic search”. Clear or change your search.", {
    exact: true,
  })).toBeVisible();

  const session = await beginPractice(page);
  expect(session.total).toBe(26);
  expect(session.items).toHaveLength(20);
  expect(new Set(session.items.map((item) => item.fact.id)).size).toBe(20);
  expect(session.items.some((item) => item.fact.id !== facts[0].id)).toBeTruthy();
  await expect(page.getByRole("heading", {
    name: session.items[0].practice.prompt,
    exact: true,
  })).toBeVisible();
  await expect(page.getByRole("combobox")).toHaveCount(0);
});

test("matching gates ratings, handles mistakes and saves all four outcomes durably", async ({ app, page }) => {
  const facts: Fact[] = [];
  for (let index = 0; index < 4; index += 1) {
    facts.push(await app.createFact({
      prompt: `Synthetic matching question ${index}?`,
      answer: `Synthetic matching answer ${index}.`,
      context: `Synthetic matching context ${index}.`,
      source: `https://example.invalid/matching/${index}`,
    }));
  }
  await freezeClock(page);
  await page.goto("/");
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Match pairs", exact: true }).click();
  const game = page.getByRole("region", { name: "Matching pairs game" });
  await expect(game.getByText("0 of 4 matched", { exact: true })).toBeVisible();
  await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toHaveCount(0);
  for (const fact of facts) {
    await expect(game.getByRole("button", { name: fact.answer, exact: true })).toBeDisabled();
  }

  const outcomes: Outcome[] = ["again", "hard", "good", "easy"];
  const saved: Review[] = [];
  for (const [index, fact] of facts.entries()) {
    await game.getByRole("button", { name: fact.prompt, exact: true }).click();
    if (index === 0) {
      await game.getByRole("button", { name: facts[1].answer, exact: true }).click();
      await expect(game.getByRole("status")).toHaveText("Not a match. Try another answer.");
      await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toHaveCount(0);
      expect((await reviewHistory(app, fact.id)).total).toBe(0);
    }
    await page.clock.fastForward(1_500);
    await game.getByRole("button", { name: fact.answer, exact: true }).click();
    await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toBeVisible();
    await expect(game.getByText(fact.context, { exact: true })).toBeVisible();
    await expect(game.getByRole("link", { name: fact.source, exact: true })).toHaveAttribute("href", fact.source);
    if (index === 0) {
      await expect(game.getByText("1 incorrect choice. Choose Again if you guessed.", { exact: true })).toBeVisible();
    }
    for (const item of facts) {
      await expect(game.getByRole("button", { name: item.prompt })).toBeDisabled();
      await expect(game.getByRole("button", { name: item.answer })).toBeDisabled();
    }
    await page.clock.fastForward(7_000);
    const review = await rate(page, outcomes[index]);
    expect(review).toMatchObject({ fact_id: fact.id, practice_id: null, method: "matching" });
    saved.push(review);
    await expect(game.getByText(`${index + 1} of 4 matched`, { exact: true })).toBeVisible();
    await expect(game.getByRole("heading", { name: "Rate your recognition", exact: true })).toHaveCount(0);
  }

  await expect(game.getByRole("status")).toHaveText("Round complete.");
  await game.getByRole("button", { name: "Play again", exact: true }).click();
  await expect(game.getByText("0 of 4 matched", { exact: true })).toBeVisible();
  await app.restart();
  for (const review of saved) {
    const durable = await reviewHistory(app, review.fact_id);
    expect(durable.total).toBe(1);
    expect(durable.items).toEqual([review]);
  }
});
