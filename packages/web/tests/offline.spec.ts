import type { Page } from "@playwright/test";
import { test, expect, type TestApp } from "./fixtures";
import { startExtraPractice } from "./actions";
import { png, wav } from "./media-files";

const header = (page: Page) => page.getByRole("banner");
const tab = (page: Page, name: string) =>
  page.getByRole("navigation", { name: "Main navigation" }).getByRole("button", { name, exact: true });

async function reviews(app: TestApp, factId: number): Promise<{ total: number; items: { outcome: string }[] }> {
  return (await app.request.get(`/api/profiles/1/facts/${factId}/reviews`)).json();
}

async function exposures(app: TestApp): Promise<number> {
  const page = await (await app.request.get("/api/profiles/1/study/events?after=0&limit=100")).json();
  return page.items.filter((event: { kind: string }) => event.kind === "exposure").length;
}

async function practiceFact(app: TestApp, prompt = "Which synthetic answer waits for the network?") {
  return app.createFact({
    prompt,
    answer: "The offline answer",
    practices: [{ kind: "flashcard", prompt, answer: "The offline answer" }],
  });
}

test("extra practice carries on offline and sends its saves on reconnecting", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();

  await context.setOffline(true);
  await expect(header(page)).toContainText("Offline");
  await page.getByLabel("Your answer").fill("My offline attempt");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  // The exposure is kept on this device, which is enough to show the answer.
  await expect(page.getByText("The offline answer", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Good Correct", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  await expect(header(page)).toContainText("Offline · 2 saves to send");
  await expect(page.getByText("2 saves to send when you are back online.", { exact: true })).toBeVisible();
  await expect(tab(page, "Library")).toBeEnabled();
  expect((await reviews(app, fact.id)).total).toBe(0);

  await context.setOffline(false);
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  await expect(header(page)).not.toContainText("Offline");
  expect((await reviews(app, fact.id)).items).toMatchObject([{ outcome: "good" }]);
  expect(await exposures(app)).toBe(1);
});

test("saves made offline reach the server after the app is closed and opened again", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
  await context.setOffline(true);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await page.getByRole("button", { name: "Easy Correct, without effort", exact: true }).click();
  await expect(header(page)).toContainText("Offline · 2 saves to send");
  await page.close();

  await context.setOffline(false);
  const reopened = await context.newPage();
  await reopened.goto("/app");
  await expect(reopened.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
  await expect.poll(async () => (await reviews(app, fact.id)).items).toMatchObject([{ outcome: "easy" }]);
  expect(await exposures(app)).toBe(1);
});

test("a second open tab leaves the first tab's waiting saves alone", async ({ app, context, page }) => {
  const fact = await practiceFact(app);
  await page.goto("/app");
  await startExtraPractice(page);
  await context.setOffline(true);
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await page.getByRole("button", { name: "Hard Correct, with difficulty", exact: true }).click();
  await expect(header(page)).toContainText("2 saves to send");

  await context.setOffline(false);
  const other = await context.newPage();
  await other.goto("/app");
  await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  await expect.poll(async () => (await reviews(app, fact.id)).total).toBe(1);
  expect(await exposures(app)).toBe(1);
});

test("the header shows Syncing while a save takes a moment", async ({ app, page }) => {
  const fact = await practiceFact(app);
  let release!: () => void;
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route(/\/api\/profiles\/1\/reviews$/, async (route) => {
    await held;
    await route.continue();
  });
  try {
    await page.goto("/app");
    await startExtraPractice(page);
    await expect(page.getByRole("heading", { name: fact.prompt, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
    await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    await expect(header(page).getByText("Syncing…", { exact: true })).toBeVisible();
    release();
    await expect(header(page).getByText("Syncing…", { exact: true })).toHaveCount(0);
    await expect(page.getByText("1 review saved.", { exact: true })).toBeVisible();
  } finally {
    release();
  }
});

/** Waits for the device's copies of the library and study, which download once the app opens. */
async function copied(page: Page) {
  await page.waitForResponse((response) => new URL(response.url()).pathname.endsWith("/study/prepared"));
  await expect(header(page).getByText("Syncing…", { exact: true })).toHaveCount(0);
}

test("the library pages and searches its copy on this device when offline", async ({ app, context, page }) => {
  for (let index = 0; index < 29; index += 1)
    await app.createFact({ prompt: `Synthetic offline question ${index}?`, answer: `Answer ${index}` });
  const bird = await app.createFact({ prompt: "What does KERERŪ mean?", answer: "Wood pigeon", tags: ["birds"] });
  const loaded = copied(page);
  await page.goto("/app");
  await expect(header(page)).toContainText("30 facts");
  await loaded;

  await context.setOffline(true);
  const search = page.getByRole("searchbox");
  // Case and macrons fold as the server's search does, and tags match too.
  await search.fill("kererū");
  const list = page.getByRole("region", { name: "Saved facts" });
  await expect(list.getByRole("heading", { level: 2 })).toHaveText([bird.prompt]);
  await search.fill("BIRDS");
  await expect(list.getByRole("heading", { level: 2 })).toHaveText([bird.prompt]);
  await search.fill("");
  await expect(header(page)).toContainText("Offline");
  // The first page ends at question 5; scrolling to it brings the next page from the copy.
  await list.getByRole("heading", { name: "Synthetic offline question 5?", exact: true }).scrollIntoViewIfNeeded();
  await expect(list.getByRole("heading", { level: 2 })).toHaveCount(30);

  // Opening a fact records its exposure on the device and shows the answer.
  await list.getByRole("heading", { name: bird.prompt, exact: true }).click();
  await expect(page.getByText("Wood pigeon", { exact: true })).toBeVisible();
  await context.setOffline(false);
  await expect.poll(() => exposures(app)).toBe(1);
});

test("the copy follows a fact added in the app", async ({ page }) => {
  await page.goto("/app");
  await page.getByRole("banner").getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByLabel("Question").fill("A fact added before going offline?");
  await page.getByLabel("Answer").fill("Kept on the device");
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect.poll(() => page.evaluate(async () => {
    const db: IDBDatabase = await new Promise((resolve) => {
      const request = indexedDB.open("keepsake");
      request.onsuccess = () => resolve(request.result);
    });
    const copy = await new Promise<{ facts: { prompt: string }[] } | undefined>((resolve) => {
      const request = db.transaction("copies").objectStore("copies").get("library:1");
      request.onsuccess = () => resolve(request.result);
    });
    return copy?.facts.map((fact) => fact.prompt);
  })).toEqual(["A fact added before going offline?"]);
});

test("extra practice keeps going from the device's copy for as long as the learner wants", async ({ app, context, page }) => {
  const facts = [
    await practiceFact(app, "Which bird is called Kererū?"),
    await practiceFact(app, "Which bird is called Tūī?"),
  ];
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await context.setOffline(true);
  await startExtraPractice(page);
  // Each fact has one flashcard, so a round has two questions.
  for (let round = 0; round < 2; round += 1) {
    for (let question = 0; question < 2; question += 1) {
      await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
      await page.getByRole("button", { name: "Good Correct", exact: true }).click();
    }
    await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
    if (round === 0) await page.getByRole("button", { name: "Practise again", exact: true }).click();
  }
  await expect(header(page)).toContainText("8 saves to send");
  await context.setOffline(false);
  await expect.poll(async () => (await reviews(app, facts[0].id)).total + (await reviews(app, facts[1].id)).total)
    .toBe(4);
});

test("matching plays rounds from the device's copy offline", async ({ app, context, page }) => {
  const facts = [];
  for (const [prompt, answer] of [["kererū", "Wood pigeon"], ["tūī", "Parson bird"], ["pīwakawaka", "Fantail"], ["kōtare", "Kingfisher"]])
    facts.push(await app.createFact({ prompt: `What does ${prompt} mean?`, answer, tags: ["birds"] }));
  // Not matchable: no topic in common with the rest.
  await app.createFact({ prompt: "What is the capital?", answer: "Wellington", tags: ["places"] });
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await context.setOffline(true);
  await tab(page, "Match pairs").click();
  const game = page.getByRole("region", { name: "Matching pairs game" });
  await expect(game.getByText("0 of 4 matched", { exact: true })).toBeVisible();
  for (const fact of facts) {
    await game.getByRole("button", { name: fact.prompt, exact: true }).click();
    await game.getByRole("button", { name: fact.answer, exact: true }).click();
  }
  await game.getByRole("button", { name: "Good Correct", exact: true }).click();
  await expect(game.getByRole("status")).toHaveText("Round complete.");
  await context.setOffline(false);
  for (const fact of facts)
    await expect.poll(async () => (await reviews(app, fact.id)).items).toMatchObject([{ outcome: "good" }]);
});

test("offline practice leaves out questions whose images are not on the device", async ({ app, context, page }) => {
  const upload = await app.request.post("/api/profiles/1/media", { headers: { "content-type": "image/png" }, data: png(20, 20) });
  const { media_id } = (await upload.json()) as { media_id: number };
  await app.createFact({ prompt: "Which bird is pictured?", answer: "Kererū", media: [{ media_id, alt: "A pigeon" }] });
  const plain = await app.createFact({ prompt: "Which bird has a white tuft?", answer: "Tūī" });
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await expect.poll(() => page.evaluate(async () => (await (await caches.open("keepsake-media")).keys()).length)).toBe(1);
  await page.evaluate(() => caches.delete("keepsake-media"));
  await context.setOffline(true);
  await startExtraPractice(page);
  await expect(page.getByText("Flashcards · 1 of 1", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: plain.prompt, exact: true })).toBeVisible();
  await context.setOffline(false);
});

type StudyEvent = { kind: string; qualifying: boolean; occurred_at: number; ignored: string | null };

async function studyEvents(app: TestApp): Promise<StudyEvent[]> {
  return (await (await app.request.get("/api/profiles/1/study/events?after=0&limit=100")).json()).items;
}

/** Opens scheduled recall afresh: the Practise tab may show a queue preloaded online. */
async function scheduledOffline(page: Page) {
  await startExtraPractice(page);
  await page.getByRole("button", { name: "Scheduled recall", exact: true }).click();
  await expect(page.getByText("Scheduled recall · prepared for offline", { exact: true })).toBeVisible();
}

async function recallAndRate(page: Page, rating: RegExp) {
  await page.getByRole("textbox", { name: "Your answer optional" }).fill("My offline attempt");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Rate your attempt", exact: true })).toBeVisible();
  await page.getByRole("button", { name: rating }).click();
}

test("scheduled recall runs offline from what was prepared, and counts from when it happened", async ({ app, context, page }) => {
  const facts = [await app.createFact({ prompt: "Synthetic first scheduled question?" }),
    await app.createFact({ prompt: "Synthetic second scheduled question?" })];
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await context.setOffline(true);
  const studiedAt = Math.floor(Date.now() / 1000);
  await scheduledOffline(page);
  await expect(page.getByText(/^Flashcards · 1 of 2$/)).toBeVisible();
  // The intervals were prepared with the question.
  await page.getByRole("textbox", { name: "Your answer optional" }).fill("My offline attempt");
  await page.getByRole("button", { name: "Reveal answer", exact: true }).click();
  await expect(page.getByRole("button", { name: /^Good\b.*\d+ days?$/ })).toBeVisible();
  await page.getByRole("button", { name: /^Good\b/ }).click();
  await recallAndRate(page, /^Again\b/);
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();
  await expect(header(page)).toContainText("4 saves to send");

  // Studied targets are not offered again offline.
  await scheduledOffline(page);
  await expect(page.getByRole("heading", { name: "Nothing due right now", exact: true })).toBeVisible();

  await context.setOffline(false);
  await expect.poll(async () => (await studyEvents(app)).length).toBe(4);
  const recorded = await studyEvents(app);
  expect(recorded.map((event) => [event.kind, event.qualifying, event.ignored])).toEqual([
    ["exposure", false, null], ["attempt", true, null], ["exposure", false, null], ["attempt", true, null],
  ]);
  for (const event of recorded) {
    expect(event.occurred_at).toBeGreaterThanOrEqual(studiedAt);
    expect(event.occurred_at).toBeLessThanOrEqual(Math.floor(Date.now() / 1000));
  }
  for (const fact of facts) expect((await reviews(app, fact.id)).total).toBe(1);
});

test("an offline attempt overtaken on another device is kept but does not count", async ({ app, context, page }) => {
  const fact = await app.createFact({ prompt: "Synthetic contested question?" });
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await context.setOffline(true);
  await scheduledOffline(page);
  await recallAndRate(page, /^Good\b/);
  await expect(page.getByRole("heading", { name: "Session complete", exact: true })).toBeVisible();

  // Meanwhile another device reviews it online.
  const queue = await (await app.request.get("/api/profiles/1/study/queue")).json();
  const question = queue.items[0];
  const elsewhere = await app.request.post("/api/profiles/1/study/events", { data: {
    submission_id: crypto.randomUUID(), target_id: question.target_id, practice_id: question.practice.id,
    content_revision: question.content_revision, expected_version: question.schedule_version,
    kind: "attempt", context: "scheduled", method: "flashcard", outcome: "hard", response_ms: 3000,
    attempted: true, assisted: false, prior_exposure: false,
  } });
  expect(elsewhere.status(), await elsewhere.text()).toBe(201);

  await context.setOffline(false);
  await expect(page.getByRole("status").filter({ hasText: "did not count" }))
    .toHaveText("1 attempt made offline did not count, because its fact changed first.");
  const recorded = await studyEvents(app);
  expect(recorded.map((event) => [event.kind, event.qualifying, Boolean(event.ignored)])).toEqual([
    ["attempt", true, false], ["exposure", false, true], ["attempt", false, true],
  ]);
  expect((await reviews(app, fact.id)).items).toMatchObject([{ outcome: "hard" }]);
});

test("scheduled recall offline asks for one connected visit when nothing is prepared", async ({ app, context, page }) => {
  await app.createFact();
  const loaded = copied(page);
  await page.goto("/app");
  await loaded;
  await page.evaluate(async () => {
    const db: IDBDatabase = await new Promise((resolve) => {
      const request = indexedDB.open("keepsake");
      request.onsuccess = () => resolve(request.result);
    });
    await new Promise((resolve) => {
      const request = db.transaction("copies", "readwrite").objectStore("copies").delete("study:1");
      request.onsuccess = resolve;
    });
  });
  await context.setOffline(true);
  await startExtraPractice(page);
  await page.getByRole("button", { name: "Scheduled recall", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "Scheduled recall offline needs Keepsake opened once with a connection, to prepare it. Extra practice and Match pairs work offline.",
  );
  await context.setOffline(false);
});

test.describe("with the service worker", () => {
  // WebKit's test browser cannot reload a page offline through a service worker.
  test.skip(({ browserName }) => browserName === "webkit", "offline reload needs Chromium");
  test.use({ serviceWorkers: "allow" });

  async function installed(page: Page) {
    await page.goto("/app");
    await expect(page.getByRole("heading", { name: "Library", exact: true })).toBeVisible();
    await page.evaluate(() => navigator.serviceWorker.ready);
    await expect.poll(() => page.evaluate(() => Boolean(navigator.serviceWorker.controller))).toBe(true);
  }

  test("the installed app opens its last profile offline", async ({ context, page }) => {
    await installed(page);
    await context.setOffline(true);
    await page.reload();
    await expect(page.getByLabel("Profile: Josiah", { exact: true })).toBeVisible();
    await expect(header(page)).toContainText("Offline");
    await context.setOffline(false);
  });

  test("images and clips are kept on the device and play offline", async ({ app, context, page }) => {
    const upload = async (type: string, data: Buffer) => {
      const response = await app.request.post("/api/profiles/1/media", { headers: { "content-type": type }, data });
      expect(response.status(), await response.text()).toBe(201);
      return ((await response.json()) as { media_id: number }).media_id;
    };
    const image = await upload("image/png", png(24, 16));
    const clip = await upload("audio/wav", wav(400));
    const fact = await app.createFact({
      prompt: "Which bird is this?",
      answer: "Kererū",
      media: [{ media_id: image, alt: "A green pigeon" }, { media_id: clip, alt: "" }],
    });
    await installed(page);
    await expect.poll(() => page.evaluate(async () => (await (await caches.open("keepsake-media")).keys()).length))
      .toBe(2);

    await context.setOffline(true);
    await page.reload();
    await page.getByRole("heading", { name: fact.prompt, exact: true }).click();
    const picture = page.getByRole("img", { name: "A green pigeon" });
    await expect.poll(() => picture.evaluate((element: HTMLImageElement) => element.complete && element.naturalWidth))
      .toBeGreaterThan(0);
    // Audio asks for byte ranges, which the kept copy answers.
    const ranged = await page.evaluate(async (path) => {
      const response = await fetch(path, { headers: { Range: "bytes=0-9" } });
      return { status: response.status, length: (await response.arrayBuffer()).byteLength };
    }, `/api/profiles/1/media/${clip}`);
    expect(ranged).toEqual({ status: 206, length: 10 });
    await context.setOffline(false);
  });

  test("turning offline use off removes this device's copy", async ({ context, page }) => {
    await installed(page);
    await page.getByLabel("Profile: Josiah", { exact: true }).click();
    await page.getByRole("button", { name: "Profile settings", exact: true }).click();
    const toggle = page.getByLabel("Use Keepsake offline on this device");
    await expect(toggle).toBeChecked();
    await toggle.uncheck();

    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole("alert")).toContainText("Could not load profiles.");
    expect(await page.evaluate(() => caches.has("keepsake-media"))).toBe(false);
    await context.setOffline(false);
    await page.reload();
    await expect(page.getByLabel("Use Keepsake offline on this device")).not.toBeChecked();
  });
});
