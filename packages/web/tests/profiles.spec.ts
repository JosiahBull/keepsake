import type { Page } from "@playwright/test";
import { test, expect, type TestApp } from "./fixtures";

async function partner(app: TestApp) {
  const response = await app.request.post("/api/profiles", {
    data: { name: "Partner", handle: "partner" },
  });
  expect(response.status(), await response.text()).toBe(201);
  return (await response.json()) as { id: number; name: string };
}

async function switchTo(page: Page, name: string) {
  await page.getByLabel(/^Profile:/).click();
  await page.getByRole("button", { name, exact: true }).click();
}

async function shareWith(page: Page, prompt: string, recipientId: number) {
  await page.getByRole("button", { name: "Select", exact: true }).click();
  await page
    .getByRole("checkbox", { name: `Select ${prompt}`, exact: true })
    .check();
  await page
    .getByRole("button", { name: "Share selected", exact: true })
    .click();
  await page
    .getByLabel("Share with", { exact: true })
    .selectOption(String(recipientId));
  await page.getByRole("button", { name: "Send facts", exact: true }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Facts shared." }),
  ).toHaveText("Facts shared.");
}

test.describe("first setup", () => {
  test.use({ profileSetup: false });
  test("names the migrated library, keeps its facts, then creates an empty partner profile", async ({
    app,
    page,
  }) => {
    const fact = await app.createFact({
      prompt: "Existing household fact?",
      answer: "Existing answer",
    });
    await page.goto("/app");
    await expect(
      page.getByRole("heading", {
        name: "Keep your existing progress",
        exact: true,
      }),
    ).toBeVisible();
    await page.getByLabel("Your name", { exact: true }).fill("Josiah");
    await page.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(page).toHaveURL(/\/p\/1\/library$/);
    await expect(
      page.getByRole("heading", { name: fact.prompt }),
    ).toBeVisible();
    await page.getByLabel(/^Profile:/).click();
    await page
      .getByRole("button", { name: "Add profile", exact: true })
      .click();
    await page.getByLabel("Your name", { exact: true }).fill("Partner");
    await page
      .getByRole("button", { name: "Create profile", exact: true })
      .click();
    await expect(page).toHaveURL(/\/p\/2\/library$/);
    await expect(
      page.getByText("No facts yet. Add a fact to start."),
    ).toBeVisible();
    await expect(page.getByRole("heading", { name: fact.prompt })).toHaveCount(
      0,
    );
    await page.reload();
    await expect(
      page.getByLabel("Profile: Partner", { exact: true }),
    ).toBeVisible();
    await switchTo(page, "Josiah");
    await expect(
      page.getByRole("heading", { name: fact.prompt }),
    ).toBeVisible();
    const target = await app.request.get(
      `/api/profiles/1/study/facts/${fact.id}`,
    );
    expect(target.ok()).toBe(true);
  });

  test("the household chooser stays put while a slow logo loads", async ({
    app,
    page,
  }) => {
    const named = await app.request.patch("/api/profiles/1", {
      data: { name: "Aroha" },
    });
    expect(named.ok(), await named.text()).toBe(true);
    await partner(app);
    let release!: () => void;
    const logo = new Promise<void>((resolve) => (release = resolve));
    await page.route(/\/keepsake-logo\.svg$/, async (route) => {
      await logo;
      await route.continue();
    });

    try {
      await page.goto("/app", { waitUntil: "commit" });
      const heading = page.getByRole("heading", {
        name: "Who’s practising?",
        exact: true,
      });
      await expect(heading).toBeVisible();
      const before = await heading.boundingBox();
      release();
      await page
        .getByRole("img", { name: "Keepsake", exact: true })
        .evaluate((image: HTMLImageElement) => image.decode());
      expect(await heading.boundingBox()).toEqual(before);
    } finally {
      release();
    }
  });
});

test("separate browser contexts accept copies while both libraries and schedules stay independent", async ({
  app,
  page,
  browser,
}) => {
  const other = await partner(app);
  const first = await app.createFact({
    prompt: "What does wai mean?",
    answer: "Water",
    source: "Synthetic household notes",
    tags: ["household"],
  });
  const second = await app.createFact({
    prompt: "What does miraka mean?",
    answer: "Milk",
    tags: ["household"],
  });
  await page.goto("/p/1/library");
  await page.getByRole("button", { name: "Select", exact: true }).click();
  await page
    .getByRole("checkbox", { name: `Select ${first.prompt}`, exact: true })
    .check();
  await page
    .getByRole("checkbox", { name: `Select ${second.prompt}`, exact: true })
    .check();
  await page
    .getByRole("button", { name: "Share selected", exact: true })
    .click();
  await page
    .getByLabel("Share with", { exact: true })
    .selectOption(String(other.id));
  await page.getByRole("button", { name: "Send facts", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/sent$/);
  await expect(
    page.getByText("Waiting for acceptance", { exact: false }),
  ).toHaveCount(2);

  const context = await browser.newContext({ baseURL: app.url });
  const receiver = await context.newPage();
  try {
    await receiver.goto(`/p/${other.id}/sharing`);
    await expect(
      receiver.getByRole("heading", { name: first.prompt, exact: true }),
    ).toBeVisible();
    await expect(receiver.getByText(first.answer, { exact: true })).toHaveCount(
      0,
    );
    const inbox = await app.request.get(
      `/api/profiles/${other.id}/shares?direction=inbox`,
    );
    const body = await inbox.json();
    expect(body.items).toHaveLength(2);
    expect(JSON.stringify(body)).not.toContain('"answer"');
    await receiver
      .getByRole("button", { name: "Select pending facts", exact: true })
      .click();
    await receiver
      .getByRole("button", { name: "Add 2 facts to my library", exact: true })
      .click();
    await expect(receiver.getByRole("status")).toContainText(
      "Copies added to your library.",
    );
    await receiver
      .getByRole("button", { name: "My library", exact: true })
      .click();
    await expect(
      receiver.getByRole("heading", { name: first.prompt, exact: true }),
    ).toBeVisible();
    const copies = await (
      await app.request.get(`/api/profiles/${other.id}/facts`)
    ).json();
    expect(copies.total).toBe(2);
    const copy = copies.items.find(
      (item: { prompt: string }) => item.prompt === first.prompt,
    );
    expect(copy.id).not.toBe(first.id);
    expect(copy.shared_by).toBe("Josiah");
    const ownSchedule = await (
      await app.request.get(`/api/profiles/1/study/facts/${first.id}`)
    ).json();
    const copySchedule = await (
      await app.request.get(`/api/profiles/${other.id}/study/facts/${copy.id}`)
    ).json();
    expect(copySchedule).not.toEqual(ownSchedule);
    expect(copySchedule).toMatchObject({
      phase: "new",
      version: 1,
      stability: null,
      difficulty: null,
      last_review_at: null,
      introduced_at: null,
      lapses: 0,
    });
    await receiver
      .getByRole("button", { name: "Practise", exact: true })
      .click();
    await receiver.getByLabel("I tried to recall", { exact: true }).check();
    await receiver
      .getByRole("button", { name: "Reveal answer", exact: true })
      .click();
    await receiver.getByRole("button", { name: /^Good\b/ }).click();
    const ownerEvents = await (
      await app.request.get("/api/profiles/1/study/events?after=0&limit=100")
    ).json();
    expect(ownerEvents.items).toEqual([]);
    const receiverEvents = await (
      await app.request.get(
        `/api/profiles/${other.id}/study/events?after=0&limit=100`,
      )
    ).json();
    expect(receiverEvents.items.length).toBeGreaterThan(0);
    await page.reload();
    await expect(
      page.getByLabel("Profile: Josiah", { exact: true }),
    ).toBeVisible();
  } finally {
    await context.close();
  }
});

test("decline and withdrawal resolve an offer without creating facts", async ({
  app,
  page,
}) => {
  const other = await partner(app);
  const fact = await app.createFact({
    prompt: "A question to decline?",
    answer: "Never shown in the inbox",
  });
  await page.goto("/p/1/library");
  await shareWith(page, fact.prompt, other.id);
  await switchTo(page, "Partner");
  await page
    .getByRole("button", { name: "Shared with me", exact: true })
    .click();
  await expect(page.getByRole("checkbox")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Select pending facts", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Add to my library", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Decline", exact: true }).click();
  await expect(page.getByText(/From Josiah · Declined/)).toBeVisible();
  expect(
    (await (await app.request.get(`/api/profiles/${other.id}/facts`)).json())
      .total,
  ).toBe(0);
  await switchTo(page, "Josiah");
  await shareWith(page, fact.prompt, other.id);
  await page.getByRole("button", { name: "Withdraw", exact: true }).click();
  await expect(page.getByText(/To Partner · Withdrawn/)).toBeVisible();
  expect(
    (await (await app.request.get(`/api/profiles/${other.id}/facts`)).json())
      .total,
  ).toBe(0);
});

test("library tabs keep the header, heading and tab bar in place", async ({
  page,
}) => {
  await page.goto("/p/1/library");
  await expect(
    page.getByText("No facts yet. Add a fact to start."),
  ).toBeVisible();
  const library = page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Library", exact: true });
  const layout = async () => ({
    navigation: await library.boundingBox(),
    heading: await page
      .getByRole("heading", { name: "Library", level: 1 })
      .boundingBox(),
    tabs: (
      await page
        .getByRole("button", { name: "My library", exact: true })
        .boundingBox()
    )?.y,
    add: await page
      .getByRole("button", { name: "Add a fact", exact: true })
      .boundingBox(),
  });
  const before = await layout();
  for (const name of ["Shared with me", "Sent facts"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(
      page.getByRole("button", { name, exact: true }),
    ).toHaveAttribute("aria-current", "page");
    await expect(library).toHaveAttribute("aria-current", "page");
    expect(await layout()).toEqual(before);
  }
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(
    page.getByRole("region", { name: "Add a fact", exact: true }),
  ).toBeVisible();
});

test("at every width the tab bar runs along the bottom and Back sits at the top left", async ({ page }) => {
  await page.goto("/p/1/library");
  const viewport = page.viewportSize()!;
  const tabs = (await page.getByRole("navigation", { name: "Main navigation" }).boundingBox())!;
  expect(Math.round(tabs.x)).toBe(0);
  expect(Math.round(tabs.width)).toBe(viewport.width);
  expect(Math.round(tabs.y + tabs.height)).toBe(viewport.height);
  const bar = (await page.getByRole("banner").boundingBox())!;
  expect(Math.round(bar.y)).toBe(0);

  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await page.getByRole("button", { name: "Profile settings", exact: true }).click();
  const back = (await page.getByRole("button", { name: "Back to Library", exact: true }).boundingBox())!;
  expect(back.x).toBeLessThan(viewport.width / 4);
  expect(back.y + back.height).toBeLessThanOrEqual(bar.y + bar.height);
});

test("the profile menu's chevron is centred on the profile name", async ({
  page,
  isMobile,
}) => {
  test.skip(isMobile, "Phones show the profile as an avatar, without a chevron.");
  await page.goto("/p/1/library");
  const summary = page.getByLabel("Profile: Josiah", { exact: true });
  const name = await summary.getByText("Josiah", { exact: true }).boundingBox();
  const chevron = await summary.locator("svg").boundingBox();
  expect(name).not.toBeNull();
  expect(chevron).not.toBeNull();
  const middle = (box: { y: number; height: number }) => box.y + box.height / 2;
  expect(Math.abs(middle(chevron!) - middle(name!))).toBeLessThanOrEqual(1);
});

test("switching confirms a typed answer, then clears queues, feedback and exposed answers", async ({
  app,
  page,
}) => {
  const other = await partner(app);
  const fact = await app.createFact({
    prompt: "Josiah-only question?",
    answer: "Josiah-only answer",
  });
  await page.goto("/p/1/practice");
  await expect(page.getByRole("heading", { name: fact.prompt })).toBeVisible();
  await page.getByLabel("Your answer").fill("An unfinished answer");
  await switchTo(page, "Partner");
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Keep working" }).click();
  await expect(page.getByLabel("Your answer")).toHaveValue(
    "An unfinished answer",
  );
  await page.getByRole("button", { name: "Partner", exact: true }).click();
  await dialog.getByRole("button", { name: "Discard and continue" }).click();
  await expect(page).toHaveURL(new RegExp(`/p/${other.id}/library$`));
  await expect(page.getByText(fact.prompt, { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await expect(
    page.getByText("Nothing due right now", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Your answer")).toHaveCount(0);
  await switchTo(page, "Josiah");
  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await expect(page.getByLabel("Your answer")).toHaveValue("");
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
});

test("failed activity writes prevent profile switching until the identical retry is saved", async ({
  app,
  page,
}) => {
  await partner(app);
  const fact = await app.createFact({
    prompt: "A retry-only question?",
    answer: "Retry answer",
  });
  let fail = true;
  const submissions: unknown[] = [];
  await page.route(/\/api\/profiles\/1\/study\/events$/, async (route) => {
    submissions.push(route.request().postDataJSON());
    if (fail)
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({ error: "Synthetic retry failure" }),
      });
    else await route.continue();
  });
  await page.goto("/p/1/practice");
  await page.getByLabel("I tried to recall", { exact: true }).check();
  await page
    .getByRole("button", { name: "Reveal answer", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "Synthetic retry failure",
  );
  await expect(page.getByText(fact.answer, { exact: true })).toHaveCount(0);
  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Partner", exact: true }),
  ).toBeDisabled();
  fail = false;
  await page.getByRole("button", { name: "Retry save", exact: true }).click();
  await expect(page.getByText(fact.answer, { exact: true })).toBeVisible();
  expect(submissions).toHaveLength(2);
  expect(submissions[0]).toEqual(submissions[1]);
  await page.getByRole("button", { name: "Partner", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/2\/library$/);
});

test("a slow library response cannot populate another profile after switching", async ({
  app,
  page,
}) => {
  await partner(app);
  const fact = await app.createFact({
    prompt: "A delayed private question?",
    answer: "Delayed answer",
  });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => (release = resolve));
  let intercepted!: () => void;
  const started = new Promise<void>((resolve) => (intercepted = resolve));
  await page.route(/\/api\/profiles\/1\/facts\?/, async (route) => {
    const response = await route.fetch();
    intercepted();
    await gate;
    await route.fulfill({ response }).catch(() => {
      /* The old profile request was aborted. */
    });
  });
  try {
    await page.goto("/p/1/library");
    await started;
    await switchTo(page, "Partner");
    await expect(
      page.getByText("No facts yet. Add a fact to start."),
    ).toBeVisible();
    release();
    await expect(page.getByRole("heading", { name: fact.prompt })).toHaveCount(
      0,
    );
    await expect(page.getByRole("alert")).toHaveCount(0);
  } finally {
    release();
  }
});

test("tab URLs stay authoritative while another tab switches profiles and settings stay personal", async ({
  app,
  page,
  context,
}) => {
  const other = await partner(app);
  const fact = await app.createFact({
    prompt: "Only the first profile knows this?",
    answer: "Separate facts",
  });
  await page.goto("/p/1/library");
  const second = await context.newPage();
  try {
    await second.goto(`/p/${other.id}/library`);
    await second.getByLabel("Profile: Partner", { exact: true }).click();
    await second
      .getByRole("button", { name: "Profile settings", exact: true })
      .click();
    await second.getByLabel("Name", { exact: true }).fill("Aroha");
    await second.getByLabel("New facts per day", { exact: true }).fill("2");
    await second
      .getByLabel("Daily study budget (minutes)", { exact: true })
      .fill("10");
    await second
      .getByRole("button", { name: "Save changes", exact: true })
      .click();
    await expect(second.getByRole("status")).toHaveText("Changes saved.");
    await expect(
      second.getByLabel("Profile: Aroha", { exact: true }),
    ).toBeVisible();
    await expect(page).toHaveURL(/\/p\/1\/library$/);
    await expect(
      page.getByRole("heading", { name: fact.prompt }),
    ).toBeVisible();
    await page.reload();
    await expect(
      page.getByLabel("Profile: Josiah", { exact: true }),
    ).toBeVisible();
    const first = await (
      await app.request.get("/api/profiles/1/study/settings")
    ).json();
    const changed = await (
      await app.request.get(`/api/profiles/${other.id}/study/settings`)
    ).json();
    expect(first.new_limit).toBe(5);
    expect(changed).toMatchObject({ new_limit: 2, daily_budget_seconds: 600 });
  } finally {
    await second.close();
  }
});

test("an unknown profile URL fails closed and offers a household choice", async ({
  app,
  page,
}) => {
  const fact = await app.createFact({
    prompt: "Never fall back to this library?",
    answer: "No fallback",
  });
  await page.goto("/p/999999/library");
  await expect(page.getByRole("alert")).toContainText(
    "This profile does not exist.",
  );
  await expect(page.getByRole("heading", { name: fact.prompt })).toHaveCount(0);
  await page.getByRole("button", { name: "Josiah", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("heading", { name: fact.prompt })).toBeVisible();
});

test("a lost settings response can be reconciled by explicitly reloading the saved values", async ({
  app,
  page,
}) => {
  await partner(app);
  let loseResponse = true;
  await page.route(/\/api\/profiles\/1\/study\/settings$/, async (route) => {
    if (route.request().method() === "PUT" && loseResponse) {
      loseResponse = false;
      await route.fetch(); // Commit the versioned update, then lose its response.
      await route.abort("failed");
    } else await route.continue();
  });
  await page.goto("/p/1/settings");
  await page.getByLabel("New facts per day", { exact: true }).fill("3");
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Could not save all changes.",
  );
  await page.getByLabel("Profile: Josiah", { exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Partner", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Discard changes and reload", exact: true })
    .click();
  await expect(
    page.getByLabel("New facts per day", { exact: true }),
  ).toHaveValue("3");
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "Partner", exact: true }).click();
  await expect(page).toHaveURL(/\/p\/2\/library$/);
});

test("back navigation retains an unsaved answer until its intended destination is confirmed", async ({
  app,
  page,
}) => {
  await partner(app);
  const fact = await app.createFact({
    prompt: "A question before going back?",
    answer: "Answer",
  });
  await page.goto("/p/1/library");
  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await expect(page.getByRole("heading", { name: fact.prompt })).toBeVisible();
  await page.getByLabel("Your answer").fill("Draft before going back");
  await page.goBack();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await expect(page.getByLabel("Your answer")).toHaveValue(
    "Draft before going back",
  );
  await page
    .getByRole("button", { name: "Discard and continue", exact: true })
    .click();
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(
    page.getByRole("heading", { name: "Library", exact: true }),
  ).toBeVisible();
});

test("creating a profile only needs a name, including macrons, handle collisions and non-Latin names", async ({
  app,
  page,
}) => {
  const existing = await app.request.post("/api/profiles", {
    data: { name: "Maia", handle: "maia" },
  });
  expect(existing.status()).toBe(201);
  await page.goto("/p/1/library");
  for (const [name, handle] of [
    ["Māia", "maia-2"],
    ["李", "profile"],
  ]) {
    await page.getByLabel(/^Profile:/).click();
    await page
      .getByRole("button", { name: "Add profile", exact: true })
      .click();
    await expect(page.getByRole("textbox")).toHaveCount(1);
    await page.getByLabel("Your name", { exact: true }).fill(name);
    await page
      .getByRole("button", { name: "Create profile", exact: true })
      .click();
    await expect(
      page.getByLabel(`Profile: ${name}`, { exact: true }),
    ).toBeVisible();
    const profiles = await (await app.request.get("/api/profiles")).json();
    expect(
      profiles.find((profile: { name: string }) => profile.name === name),
    ).toMatchObject({ name, handle });
  }
});

test("a lost profile creation response opens the one created library without duplicating it", async ({
  app,
  page,
}) => {
  let creates = 0;
  await page.route(/\/api\/profiles$/, async (route) => {
    if (route.request().method() === "POST") {
      creates += 1;
      await route.fetch();
      await route.abort("failed");
    } else await route.continue();
  });
  await page.goto("/p/1/library");
  await page.getByLabel(/^Profile:/).click();
  await page.getByRole("button", { name: "Add profile", exact: true }).click();
  await page.getByLabel("Your name", { exact: true }).fill("Partner");
  await page
    .getByRole("button", { name: "Create profile", exact: true })
    .click();
  await expect(
    page.getByLabel("Profile: Partner", { exact: true }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/p\/2\/library$/);
  const profiles = await (await app.request.get("/api/profiles")).json();
  expect(profiles).toHaveLength(2);
  expect(
    profiles.filter(
      (profile: { handle: string }) => profile.handle === "partner",
    ),
  ).toHaveLength(1);
  expect(creates).toBe(1);
});
