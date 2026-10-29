import type { Page } from "@playwright/test";
import type { components } from "@keepsake/client";
import { test, expect, type Fact } from "./fixtures";
import { sounds, startExtraPractice } from "./actions";

type PracticeSession = components["schemas"]["PracticeSession"];

async function practise(page: Page) {
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Practise", exact: true }).click();
}

async function recall(page: Page) {
  await page.getByRole("textbox", { name: "Your answer optional" }).fill("My synthetic attempt");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
}

test("recall flips each answer, files every rating alike and completes after the last save", async ({ app, page }) => {
  for (let index = 0; index < 4; index += 1) {
    await app.createFact({ prompt: `Synthetic sound question ${index}?`, answer: `Synthetic sound answer ${index}.` });
  }
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let attempts = 0;
  await page.route(/\/api\/profiles\/1\/study\/events$/, async (route) => {
    if (route.request().postDataJSON().kind === "attempt" && ++attempts === 4) await held;
    await route.continue();
  });

  try {
    await page.goto("/");
    await practise(page);
    // A rewarding sound for Good or Easy would reward the rating rather than the recall.
    for (const rating of ["Again", "Hard", "Good", "Easy"]) {
      await recall(page);
      await page.getByRole("button", { name: new RegExp(`^${rating}\\b`) }).click();
    }
    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    await expect(page.getByRole("status")).toHaveText("Saving…");
    const reviews = ["flip", "file", "flip", "file", "flip", "file", "flip", "file"];
    expect(await sounds(page)).toEqual(reviews);

    release();
    await expect(page.getByText("4 reviews saved.", { exact: true })).toBeVisible();
    await expect.poll(() => sounds(page)).toEqual([...reviews, "done"]);
  } finally {
    release();
  }
});

test("only multiple choice sounds right or wrong when its answer is shown", async ({ app, page }) => {
  const choices = ["The fictional planet Luma", "The fictional planet Neri", "The fictional planet Sora"];
  for (const index of [0, 1]) {
    await app.createFact({
      prompt: `Which synthetic planet ${index}?`,
      answer: choices[0],
      practices: [{
        kind: "multiple_choice",
        prompt: `Choose synthetic planet ${index}.`,
        answer: choices[0],
        choices,
      }],
    });
  }
  await page.goto("/");
  const loaded = page.waitForResponse((response) =>
    new URL(response.url()).pathname === "/api/profiles/1/practice");
  await startExtraPractice(page);
  const session: PracticeSession = await (await loaded).json();
  expect(session.items).toHaveLength(4);

  const expected: string[] = [];
  let wrong = true;
  for (const question of session.items) {
    await expect(page.getByRole("heading", { name: question.practice.prompt, exact: true })).toBeVisible();
    if (question.practice.kind === "multiple_choice") {
      await page.getByRole("radio", { name: wrong ? choices[1] : choices[0], exact: true }).check();
      await page.getByRole("button", { name: "Check answer", exact: true }).click();
      expected.push(wrong ? "miss" : "right");
      wrong = false;
    } else {
      await recall(page);
      expected.push("flip");
    }
    await page.getByRole("button", { name: /^Good\b/ }).click();
    expected.push("file");
  }
  await expect.poll(() => sounds(page)).toEqual([...expected, "done"]);
});

test("matching ticks a question, knocks a wrong answer and completes on the last rating", async ({ app, page }) => {
  const facts: Fact[] = [];
  for (let index = 0; index < 3; index += 1) {
    facts.push(await app.createFact({
      prompt: `Synthetic matching question ${index}?`,
      answer: `Synthetic matching answer ${index}.`,
      tags: ["synthetic-sound-bank"],
    }));
  }
  await page.goto("/");
  await page.getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Match pairs", exact: true }).click();
  const game = page.getByRole("region", { name: "Matching pairs game" });
  await expect(game.getByText("0 of 3 matched", { exact: true })).toBeVisible();

  for (const [index, fact] of facts.entries()) {
    await game.getByRole("button", { name: fact.prompt, exact: true }).click();
    if (index === 0) await game.getByRole("button", { name: facts[1].answer, exact: true }).click();
    await game.getByRole("button", { name: fact.answer, exact: true }).click();
    await page.getByRole("button", { name: /^Good\b/ }).click();
  }
  await expect(game.getByRole("status")).toHaveText("Round complete.");
  expect(await sounds(page)).toEqual([
    "tick", "miss", "right", "file",
    "tick", "right", "file",
    "tick", "right", "done",
  ]);
});

test("a failed background save sounds an alert and completion waits for the retry", async ({ app, page }) => {
  await app.createFact();
  let posts = 0;
  await page.route(/\/api\/profiles\/1\/reviews$/, async (route) => {
    posts += 1;
    if (posts === 1) {
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({ error: "Temporarily unavailable." }),
      });
      return;
    }
    await route.continue();
  });

  await page.goto("/");
  await startExtraPractice(page);
  await recall(page);
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await expect(page.getByRole("alert")).toContainText("Could not save a review.");
  await expect.poll(() => sounds(page)).toEqual(["flip", "file", "alert"]);

  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  await expect.poll(() => sounds(page)).toEqual(["flip", "file", "alert", "done"]);
});

test("sounds turned off on this device stay off after a reload", async ({ app, page }) => {
  await app.createFact();
  await page.goto("/p/1/settings");
  const toggle = page.getByRole("checkbox", { name: "Play sounds on this device", exact: true });
  await expect(toggle).toBeChecked();
  await toggle.uncheck();
  await page.reload();
  await expect(toggle).not.toBeChecked();

  await practise(page);
  await recall(page);
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  expect(await sounds(page)).toEqual([]);

  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await page.getByRole("button", { name: "Profile settings", exact: true }).click();
  await toggle.check();
  expect(await sounds(page)).toEqual(["file"]);
});

test("saving and accepting facts sound like keeping them, and sharing sends them", async ({ app, page }) => {
  const created = await app.request.post("/api/profiles", { data: { name: "Partner", handle: "partner" } });
  expect(created.status(), await created.text()).toBe(201);
  const partner = (await created.json()) as { id: number };
  const prompt = "Which synthetic fact is shared?";

  await page.goto("/");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByRole("textbox", { name: "Question", exact: true }).fill(prompt);
  await page.getByRole("textbox", { name: "Answer", exact: true }).fill("The shared synthetic fact.");
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Fact saved." })).toBeVisible();

  await page.getByRole("button", { name: "Select", exact: true }).click();
  await page.getByRole("checkbox", { name: `Select ${prompt}`, exact: true }).check();
  await page.getByRole("button", { name: "Share selected", exact: true }).click();
  await page.getByLabel("Share with", { exact: true }).selectOption(String(partner.id));
  await page.getByRole("button", { name: "Send facts", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Facts shared." })).toBeVisible();
  expect(await sounds(page)).toEqual(["keep", "send"]);

  await page.getByLabel(/^Profile:/).click();
  await page.getByRole("button", { name: "Partner", exact: true }).click();
  await page.getByRole("button", { name: "Shared with me", exact: true }).click();
  await page.getByRole("button", { name: "Add to my library", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Copies added to your library.");
  expect(await sounds(page)).toEqual(["keep", "send", "keep"]);
});
