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
  await page.goto("/");
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

  await page.goto("/");
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

  await page.goto("/");
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
  await page.goto(app.url);
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

  await page.goto("/");
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

test("pagination has no missing or repeated facts and moves keyboard focus to page content", async ({ page, app }) => {
  for (let number = 1; number <= 30; number += 1) {
    await app.createFact(plainFact(`Household fact ${String(number).padStart(2, "0")}`));
  }

  await page.goto("/");
  const rows = page.getByRole("article");
  const pagination = page.getByRole("navigation", { name: "Library pages" });
  await expect(rows).toHaveCount(25);
  await expect(page.getByText("30 facts", { exact: true })).toBeVisible();
  await expect(pagination.getByRole("button", { name: "Previous", exact: true })).toBeDisabled();
  await expect(rows.first().getByRole("heading")).toHaveText("Household fact 30");
  await expect(rows.last().getByRole("heading")).toHaveText("Household fact 06");
  const firstPage = await rows.getByRole("heading").allTextContents();

  const next = pagination.getByRole("button", { name: "Next", exact: true });
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(rows).toHaveCount(5);
  await expect(pagination.getByText("Page 2", { exact: true })).toBeVisible();
  await expect(next).toBeDisabled();
  await expect(rows.first().locator("summary")).toBeFocused();
  await expect(rows.first().getByRole("heading")).toHaveText("Household fact 05");
  await expect(rows.last().getByRole("heading")).toHaveText("Household fact 01");
  const secondPage = await rows.getByRole("heading").allTextContents();
  expect(new Set([...firstPage, ...secondPage]).size).toBe(30);
  await page.keyboard.press("Space");
  await expect(rows.first().getByText("Answer for Household fact 05", { exact: true })).toBeVisible();

  await pagination.getByRole("button", { name: "Previous", exact: true }).click();
  await expect(rows).toHaveCount(25);
  await expect(rows.first().locator("summary")).toBeFocused();
  expect(await rows.getByRole("heading").allTextContents()).toEqual(firstPage);
  await expectViewportFit(page);
});

test("search filters every supported field literally, handles Māori macrons, and resets pagination", async ({ page, app }) => {
  for (let number = 1; number <= 26; number += 1) {
    await app.createFact(plainFact(`Ordinary household fact ${number}`));
  }
  await app.createFact(plainFact("Kitchen 100%_ready\\marker"));
  await app.createFact(plainFact("Plain answer question", { answer: "Distinctive answer needle" }));
  await app.createFact(plainFact("Plain context question", { context: "Distinctive context needle" }));
  await app.createFact(plainFact("Plain tag question", { tags: ["tag-needle"] }));
  await app.createFact(plainFact("ĀĒĪŌŪ around the house", { answer: "Māori vowel practice" }));
  await page.goto("/");
  const rows = page.getByRole("article");
  await expect(rows).toHaveCount(25);
  await page.getByRole("navigation", { name: "Library pages" }).getByRole("button", { name: "Next", exact: true }).click();
  await expect(page.getByText("Page 2", { exact: true })).toBeVisible();

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
    await expect(page.getByRole("navigation", { name: "Library pages" })).toHaveCount(0);
  }

  await search.fill("no matching household word");
  await expect(rows).toHaveCount(0);
  await expect(page.getByText("No facts match “no matching household word”. Clear or change your search.", { exact: true })).toBeVisible();
  await search.fill("");
  await expect(rows).toHaveCount(25);
  await expect(rows.first().getByRole("heading")).toHaveText("ĀĒĪŌŪ around the house");
  await expect(page.getByText("Page 1", { exact: true })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Library pages" }).getByRole("button", { name: "Previous", exact: true })).toBeDisabled();
});

test("a fact can be added and disclosed with the keyboard", async ({ page }) => {
  await page.goto("/");
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
