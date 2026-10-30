import type { Page } from "@playwright/test";
import { test, expect, type Fact, type NewFact } from "./fixtures";

const plainFact = (prompt: string, overrides: Partial<NewFact> = {}): NewFact => ({
  prompt,
  answer: `Answer for ${prompt}`,
  context: "",
  source: "",
  tags: [],
  practices: [],
  ...overrides,
});

async function expectViewportFit(page: Page) {
  const dimensions = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    content: document.documentElement.scrollWidth,
  }));

  expect(dimensions.content).toBeLessThanOrEqual(dimensions.viewport + 1);
}

test("required fields and cancellation leave an empty library unchanged", async ({ page, app }) => {
  await page.goto("/app");
  await expect(page.getByText("No facts yet. Add a fact to start.")).toBeVisible();
  await expect(page.getByRole("searchbox", { name: "Search facts" })).toHaveCount(0);
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();

  const question = page.getByRole("textbox", { name: "Question", exact: true });
  const answer = page.getByRole("textbox", { name: "Answer", exact: true });
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(question).toBeFocused();
  expect(await question.evaluate((element) => (element as HTMLTextAreaElement).validity.valueMissing)).toBe(true);

  await question.fill("What does wai mean?");
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(answer).toBeFocused();
  await answer.fill("Water");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Add a fact", exact: true })).toHaveCount(0);
  await expect(page.getByText("No facts yet. Add a fact to start.")).toBeVisible();

  const response = await app.request.get("/api/profiles/1/facts");
  expect(response.ok()).toBe(true);
  expect(await response.json()).toMatchObject({ total: 0, items: [] });
});

test("UTF-8 byte validation preserves the draft and allows a corrected save", async ({ page, app }) => {
  const oversizedQuestion = "ā".repeat(4001);
  const correctedQuestion = "What does māra mean?";
  const answerText = "Garden";
  const factPosts: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/profiles/1/facts") {
      factPosts.push(request.url());
    }
  });

  await page.goto("/app");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  const question = page.getByRole("textbox", { name: "Question", exact: true });
  const answer = page.getByRole("textbox", { name: "Answer", exact: true });
  await question.fill(oversizedQuestion);
  await answer.fill(answerText);
  await page.getByRole("button", { name: "Save fact", exact: true }).click();

  await expect(page.getByRole("alert")).toHaveText(
    "Could not save the fact. Question is too long. Shorten it before saving.",
  );
  await expect(question).toHaveValue(oversizedQuestion);
  await expect(answer).toHaveValue(answerText);
  expect(factPosts).toHaveLength(0);
  const emptyLibrary = await app.request.get("/api/profiles/1/facts");
  expect(emptyLibrary.ok()).toBe(true);
  expect(await emptyLibrary.json()).toMatchObject({ total: 0, items: [] });

  await question.fill(correctedQuestion);
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Fact saved." })).toHaveText("Fact saved.");
  expect(factPosts).toHaveLength(1);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("article")).toHaveCount(1);
  await page.getByRole("heading", { name: correctedQuestion, exact: true }).click();
  await expect(page.getByText(answerText, { exact: true })).toBeVisible();

  const savedLibrary = await app.request.get("/api/profiles/1/facts");
  expect(savedLibrary.ok()).toBe(true);
  expect(await savedLibrary.json()).toMatchObject({
    total: 1,
    items: [{ prompt: correctedQuestion, answer: answerText }],
  });
});

test("facts saved in the browser survive reload and restart and round-trip through the CLI", async ({ page, app }) => {
  const prompt = "What does horoi mean around the house?";
  const answer = "To wash or clean";
  const context = "Horoi ngā rīhi.\nWash the dishes.";
  const source = "https://example.com/learning/horoi?language=mi&household=true";

  await page.goto("/app");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByRole("textbox", { name: "Question", exact: true }).fill(`  ${prompt}  `);
  await page.getByRole("textbox", { name: "Answer", exact: true }).fill(`  ${answer}  `);
  await page.getByText("Context, source and tags (optional)", { exact: true }).click();
  await page.getByRole("textbox", { name: "Context", exact: true }).fill(context);
  await page.getByRole("textbox", { name: "Source", exact: true }).fill(source);
  await page.getByRole("textbox", { name: "Tags comma separated", exact: true }).fill(" household, te reo, , dishes ");
  await expectViewportFit(page);
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Fact saved." })).toHaveText("Fact saved.");
  await expect(page.getByRole("heading", { name: prompt, exact: true })).toBeVisible();

  const listed = JSON.parse(await app.cli(["--json", "list"])) as { items: Fact[]; total: number };
  expect(listed.total).toBe(1);
  expect(listed.items[0]).toMatchObject({ prompt, answer, context, source, tags: ["household", "te reo", "dishes"] });
  const shown = JSON.parse(await app.cli(["--json", "show", String(listed.items[0].id)])) as Fact;
  expect(shown).toMatchObject({ prompt, answer, context, source });

  await page.reload();
  const row = page.getByRole("article").filter({ has: page.getByRole("heading", { name: prompt, exact: true }) });
  await row.locator("summary").click();
  await expect(row.getByText(answer, { exact: true })).toBeVisible();
  await expect(row.getByText(context, { exact: true })).toBeVisible();
  await expect(row.getByText("Tags: household, te reo, dishes", { exact: true })).toBeVisible();
  await expect(row.getByRole("link", { name: source, exact: true })).toHaveAttribute("href", source);

  const imported = JSON.parse(await app.cli([
    "--json", "add", "--prompt", "What does kai mean?", "--answer", "Food",
    "--context", "Kua rite te kai. The food is ready.", "--tag", "household",
  ])) as Fact;
  expect(imported.prompt).toBe("What does kai mean?");

  await app.restart();
  await page.goto(`${app.url}/app`);
  await expect(page.getByRole("article")).toHaveCount(2);
  await expect(page.getByRole("heading", { name: prompt, exact: true })).toBeVisible();
  await page.getByRole("heading", { name: imported.prompt, exact: true }).click();
  await expect(page.getByText("Food", { exact: true })).toBeVisible();
  await expectViewportFit(page);
});

test("saved content stays text and only HTTP sources become safe external links", async ({ page, app }) => {
  const markup = '<img src=x onerror="window.keepsakeInjected=true">';
  const unsafeSource = "javascript:window.keepsakeInjected=true";
  const safeSource = `https://example.com/${"reference-".repeat(24)}?q=%3Cscript%3E`;
  await app.createFact(plainFact("Unsafe source and literal markup", {
    answer: markup,
    context: "<script>window.keepsakeInjected=true</script>",
    source: unsafeSource,
    tags: ["<b>literal tag</b>"],
  }));
  await app.createFact(plainFact("Safe source", { source: safeSource }));
  await app.createFact(plainFact("Book source", { source: "A household phrasebook, page 12" }));

  await page.goto("/app");
  const unsafe = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Unsafe source and literal markup", exact: true }) });
  await unsafe.locator("summary").click();
  await expect(unsafe.getByText(markup, { exact: true })).toBeVisible();
  await expect(unsafe.getByText("<script>window.keepsakeInjected=true</script>", { exact: true })).toBeVisible();
  await expect(unsafe.getByText(`Source: ${unsafeSource}`, { exact: true })).toBeVisible();
  await expect(unsafe.getByText("Tags: <b>literal tag</b>", { exact: true })).toBeVisible();
  await expect(unsafe.getByRole("link")).toHaveCount(0);
  await expect(unsafe.locator("img, script, b")).toHaveCount(0);
  expect(await page.evaluate(() => Reflect.get(window, "keepsakeInjected"))).toBeUndefined();

  const safe = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Safe source", exact: true }) });
  await safe.locator("summary").click();
  const link = safe.getByRole("link", { name: safeSource, exact: true });
  await expect(link).toHaveAttribute("href", safeSource);
  await expect(link).toHaveAttribute("target", "_blank");
  await expect(link).toHaveAttribute("rel", /\bnoreferrer\b/);
  const book = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Book source", exact: true }) });
  await book.locator("summary").click();
  await expect(book.getByText("Source: A household phrasebook, page 12", { exact: true })).toBeVisible();
  await expect(book.getByRole("link")).toHaveCount(0);
  await expectViewportFit(page);
});

/** Scrolls to the last listed fact until the list holds `count` facts. */
async function scrollUntilListed(page: Page, count: number) {
  const rows = page.getByRole("article");
  await expect(async () => {
    await rows.last().scrollIntoViewIfNeeded();
    await expect(rows).toHaveCount(count, { timeout: 1_000 });
  }).toPass();
}

function gate() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

test("scrolling to the end of the library lists every fact once, newest first", async ({ page, app }) => {
  const created: Fact[] = [];
  for (let number = 1; number <= 60; number += 1) {
    created.push(await app.createFact(plainFact(`Household fact ${String(number).padStart(2, "0")}`)));
  }
  const cursors: (string | null)[] = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (request.method() === "GET" && url.pathname === "/api/profiles/1/facts") {
      cursors.push(url.searchParams.get("before_id"));
    }
  });

  await page.goto("/app");
  await expect(page.getByText("60 facts", { exact: true })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Library pages" })).toHaveCount(0);
  await scrollUntilListed(page, 60);

  const rows = page.getByRole("article");
  expect(await rows.getByRole("heading").allTextContents()).toEqual(
    created.map((fact) => fact.prompt).reverse(),
  );
  // Each page starts below the last fact listed, and nothing is asked for past the end.
  expect(cursors).toEqual([null, String(created[35].id), String(created[10].id)]);
  await rows.last().locator("summary").click();
  await expect(rows.last().getByText("Answer for Household fact 01", { exact: true })).toBeVisible();
  await expectViewportFit(page);
});

test("a page that fails to load can be retried, and a late page never joins a newer search", async ({ page, app }) => {
  for (let number = 1; number <= 30; number += 1) {
    await app.createFact(plainFact(`Household fact ${String(number).padStart(2, "0")}`));
  }
  const rows = page.getByRole("article");
  let failed = false;
  await page.route(/\/api\/profiles\/1\/facts\?.*before_id=/, async (route) => {
    if (failed) {
      await route.continue();
      return;
    }
    failed = true;
    await route.fulfill({ status: 503, json: { error: "The library is busy." } });
  });

  await page.goto("/app");
  await rows.last().scrollIntoViewIfNeeded();
  const alert = page.getByRole("alert");
  await expect(alert).toHaveText(/^Could not load more facts\. The library is busy\.\s*Try again$/);
  await expect(rows).toHaveCount(25);
  await alert.getByRole("button", { name: "Try again", exact: true }).click();
  await expect(rows).toHaveCount(30);
  await expect(alert).toHaveCount(0);

  await page.unroute(/\/api\/profiles\/1\/facts\?.*before_id=/);
  const held = gate();
  await page.route(/\/api\/profiles\/1\/facts\?.*before_id=/, async (route) => {
    await held.promise;
    await route.continue();
  });
  try {
    await page.reload();
    await rows.last().scrollIntoViewIfNeeded();
    await expect(page.getByRole("status").filter({ hasText: "Loading more facts…" })).toBeVisible();
    await page.getByRole("searchbox", { name: "Search facts" }).fill("Household fact 03");
    await expect(rows).toHaveCount(1);

    const late = page.waitForResponse((response) => response.url().includes("before_id="));
    held.release();
    await late;
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(resolve)));
    await expect(rows).toHaveCount(1);
    await expect(rows.getByRole("heading")).toHaveText("Household fact 03");
  } finally {
    held.release();
  }
});

test("search filters every supported field literally, handles Māori macrons, and starts again from the newest fact", async ({ page, app }) => {
  for (let number = 1; number <= 26; number += 1) {
    await app.createFact(plainFact(`Ordinary household fact ${number}`));
  }
  await app.createFact(plainFact("Kitchen 100%_ready\\marker"));
  await app.createFact(plainFact("Plain answer question", { answer: "Distinctive answer needle" }));
  await app.createFact(plainFact("Plain context question", { context: "Distinctive context needle" }));
  await app.createFact(plainFact("Plain tag question", { tags: ["tag-needle"] }));
  await app.createFact(plainFact("ĀĒĪŌŪ around the house", { answer: "Māori vowel practice" }));
  await page.goto("/app");
  const rows = page.getByRole("article");
  await scrollUntilListed(page, 31);

  const search = page.getByRole("searchbox", { name: "Search facts" });
  for (const [query, title] of [
    ["%_ready\\", "Kitchen 100%_ready\\marker"],
    ["ANSWER NEEDLE", "Plain answer question"],
    ["context needle", "Plain context question"],
    ["TAG-NEEDLE", "Plain tag question"],
    ["āēīōū", "ĀĒĪŌŪ around the house"],
  ]) {
    await search.fill(query);
    await expect(rows).toHaveCount(1);
    await expect(rows.getByRole("heading")).toHaveText(title);
  }

  await search.fill("no matching household word");
  await expect(rows).toHaveCount(0);
  await expect(page.getByText("No facts match “no matching household word”. Clear or change your search.", { exact: true })).toBeVisible();
  await search.fill("");
  await expect(rows.first().getByRole("heading")).toHaveText("ĀĒĪŌŪ around the house");
  await page.evaluate(() => window.scrollTo(0, 0));
  await scrollUntilListed(page, 31);
});

test("a fact can be added and disclosed with the keyboard", async ({ page }) => {
  await page.goto("/app");
  const add = page.getByRole("button", { name: "Add a fact", exact: true });
  await add.focus();
  await page.keyboard.press("Enter");
  const question = page.getByRole("textbox", { name: "Question", exact: true });
  await question.focus();
  await page.keyboard.type("What does inu mean?");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("textbox", { name: "Answer", exact: true })).toBeFocused();
  await page.keyboard.type("To drink");
  await page.keyboard.press("Tab");
  // Safari skips file inputs, as it does buttons, unless full keyboard access is on.
  const media = page.getByLabel("Images and audio");
  if (await media.evaluate((element) => element === document.activeElement)) {
    await page.keyboard.press("Tab");
  }
  await expect(page.getByText("Context, source and tags (optional)", { exact: true })).toBeFocused();
  // Safari's button tab stops depend on the user's keyboard-navigation setting.
  const save = page.getByRole("button", { name: "Save fact", exact: true });
  await save.focus();
  await expect(save).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("status").filter({ hasText: "Fact saved." })).toHaveText("Fact saved.");

  const summary = page.getByRole("article").locator("summary");
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("To drink", { exact: true })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByText("To drink", { exact: true })).toBeHidden();
  await expectViewportFit(page);
});

test("disclosing a fact keeps the list still while its exposure saves", async ({ page, app }) => {
  const lower = await app.createFact({ prompt: "Synthetic lower disclosed question?" });
  const upper = await app.createFact({ prompt: "Synthetic upper disclosed question?" });
  let release!: () => void;
  const saved = new Promise<void>((resolve) => (release = resolve));
  await page.route(/\/api\/profiles\/1\/study\/exposures$/, async (route) => {
    await saved;
    await route.continue();
  });

  try {
    await page.goto("/app");
    const list = page.getByRole("region", { name: "Saved facts" });
    const below = list.getByRole("heading", { name: lower.prompt, exact: true });
    await expect(below).toBeVisible();
    const before = await below.boundingBox();
    await list.getByRole("heading", { name: upper.prompt, exact: true }).click();
    await expect(page.getByText("Saving…", { exact: true })).toBeAttached();
    expect(await below.boundingBox()).toEqual(before);
    release();
    await expect(page.getByText("Saving…", { exact: true })).toHaveCount(0);
  } finally {
    release();
  }
});

test("the app has no page scrollbar, since the library grows as it scrolls", async ({ page }) => {
  await page.goto("/app");
  await expect(page.getByText("No facts yet. Add a fact to start.")).toBeVisible();
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("scrollbar-width")))
    .toBe("none");
});
