import type { components } from "@keepsake/client";
import { test, expect, type TestApp } from "./fixtures";

type Profile = components["schemas"]["Profile"];
type FactPage = components["schemas"]["FactPage"];
type SharePage = components["schemas"]["SharePage"];
type StudyQueue = components["schemas"]["StudyQueue"];

test.use({ householdDefaults: true, profileSetup: false });

async function household(app: TestApp) {
  const response = await app.request.get("/api/profiles");
  expect(response.ok(), await response.text()).toBe(true);
  const profiles = (await response.json()) as Profile[];
  expect(profiles).toHaveLength(2);
  const josiah = profiles.find((profile) => profile.name === "Josiah")!;
  const ansam = profiles.find((profile) => profile.handle === "ansam")!;
  expect(josiah).toMatchObject({
    id: 1,
    handle: "default",
    setup_complete: true,
  });
  expect(ansam).toMatchObject({ name: "Ansam", setup_complete: true });
  return { josiah, ansam };
}

async function facts(app: TestApp, id: number) {
  const items: FactPage["items"] = [];
  let before: number | null = null;
  do {
    const response = await app.request.get(
      `/api/profiles/${id}/facts?limit=25${before === null ? "" : `&before_id=${before}`}`,
    );
    expect(response.ok(), await response.text()).toBe(true);
    const page = (await response.json()) as FactPage;
    items.push(...page.items);
    before = page.next_cursor ?? null;
  } while (before !== null);
  return items;
}

test("a fresh household has Josiah's 40 facts, Ansam's 60 facts and one-way pending shares", async ({
  app,
  page,
}) => {
  const { josiah, ansam } = await household(app);
  const josiahFacts = await facts(app, josiah.id);
  const ansamFacts = await facts(app, ansam.id);
  expect(josiahFacts).toHaveLength(40);
  expect(ansamFacts).toHaveLength(60);
  expect(
    new Set([...josiahFacts, ...ansamFacts].map((fact) => fact.id)).size,
  ).toBe(100);

  const offers: SharePage["items"] = [];
  let before: number | null = null;
  do {
    const response = await app.request.get(
      `/api/profiles/${josiah.id}/shares?direction=inbox&status=pending&limit=25${before === null ? "" : `&before_id=${before}`}`,
    );
    expect(response.ok(), await response.text()).toBe(true);
    const shares = (await response.json()) as SharePage;
    expect(shares.total).toBe(60);
    offers.push(...shares.items);
    before = shares.next_cursor ?? null;
  } while (before !== null);
  expect(offers).toHaveLength(60);
  expect(new Set(offers.map((offer) => offer.source_fact_id))).toEqual(
    new Set(ansamFacts.map((fact) => fact.id)),
  );
  for (const offer of offers) {
    expect(offer).toMatchObject({
      sender_id: ansam.id,
      recipient_id: josiah.id,
      sender_name: "Ansam",
      recipient_name: "Josiah",
      status: "pending",
    });
    expect(offer).not.toHaveProperty("answer");
    expect(offer).not.toHaveProperty("snapshot");
    expect(offer).not.toHaveProperty("practices");
  }
  for (const [id, direction] of [
    [ansam.id, "inbox"],
    [josiah.id, "sent"],
  ] as const) {
    const response = await app.request.get(
      `/api/profiles/${id}/shares?direction=${direction}`,
    );
    expect(response.ok()).toBe(true);
    expect(await response.json()).toMatchObject({ total: 0, items: [] });
  }

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Who’s practising?", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Josiah", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Ansam", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Ansam", exact: true }).click();
  await expect(page.getByText("60 facts", { exact: true })).toBeVisible();
  const listed = page
    .getByRole("region", { name: "Saved facts", exact: true })
    .locator("article");
  await expect(async () => {
    await listed.last().scrollIntoViewIfNeeded();
    await expect(listed).toHaveCount(60, { timeout: 1_000 });
  }).toPass();
  await page.getByLabel("Profile: Ansam", { exact: true }).click();
  await page.getByRole("button", { name: "Josiah", exact: true }).click();
  await expect(page.getByText("40 facts", { exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "Shared with me", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: offers[0].question, exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Show more", exact: true }),
  ).toBeEnabled();
});

test("Josiah accepts a fresh te reo copy while Ansam's library and review progress stay separate", async ({
  app,
  page,
}) => {
  const { josiah, ansam } = await household(app);
  const inbox = await app.request.get(
    `/api/profiles/${josiah.id}/shares?direction=inbox&status=pending&limit=25`,
  );
  const offer = ((await inbox.json()) as SharePage).items[0];
  const sourceSchedule = await (
    await app.request.get(
      `/api/profiles/${ansam.id}/study/facts/${offer.source_fact_id}`,
    )
  ).json();
  await page.goto(`/p/${josiah.id}/sharing`);
  const row = page
    .locator("article")
    .filter({
      has: page.getByRole("heading", { name: offer.question, exact: true }),
    });
  await row
    .getByRole("button", { name: "Add to my library", exact: true })
    .click();
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: "Copies added to your library." }),
  ).toBeVisible();
  const resolved = (
    (await (
      await app.request.get(
        `/api/profiles/${josiah.id}/shares?direction=inbox&limit=100`,
      )
    ).json()) as SharePage
  ).items.find((item) => item.id === offer.id)!;
  expect(resolved.status).toBe("accepted");
  expect(resolved.accepted_fact_id).not.toBe(offer.source_fact_id);
  expect(resolved.accepted_fact_id).toBeGreaterThan(0);
  const copied = await (
    await app.request.get(
      `/api/profiles/${josiah.id}/study/facts/${resolved.accepted_fact_id}`,
    )
  ).json();
  expect(copied).toMatchObject({
    phase: "new",
    stability: null,
    difficulty: null,
    introduced_at: null,
    last_review_at: null,
    lapses: 0,
  });
  expect(await facts(app, ansam.id)).toHaveLength(60);
  expect(await facts(app, josiah.id)).toHaveLength(41);
  const remaining = await app.request.get(
    `/api/profiles/${josiah.id}/shares?direction=inbox&status=pending`,
  );
  expect((await remaining.json()).total).toBe(59);

  await page.getByRole("button", { name: "My library", exact: true }).click();
  await expect(page.getByText("41 facts", { exact: true })).toBeVisible();
  const due = (await (
    await app.request.get(`/api/profiles/${josiah.id}/study/queue?limit=20`)
  ).json()) as StudyQueue;
  expect(due.items.length).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Practise", exact: true }).click();
  await expect(
    page.getByRole("heading", {
      name: due.items[0].practice.prompt,
      exact: true,
    }),
  ).toBeVisible();
  await page.getByLabel("I tried to recall", { exact: true }).check();
  await page
    .getByRole("button", { name: "Reveal answer", exact: true })
    .click();
  const saved = page.waitForResponse(
    (response) =>
      response.url().includes(`/api/profiles/${josiah.id}/study/events`) &&
      response.request().postDataJSON()?.kind === "attempt",
  );
  await page.getByRole("button", { name: /^Good\b/ }).click();
  expect((await (await saved).json()).qualifying).toBe(true);
  expect(
    await (
      await app.request.get(
        `/api/profiles/${ansam.id}/study/facts/${offer.source_fact_id}`,
      )
    ).json(),
  ).toEqual(sourceSchedule);
  const ansamEvents = await app.request.get(
    `/api/profiles/${ansam.id}/study/events?after=0&limit=100`,
  );
  expect((await ansamEvents.json()).items).toEqual([]);

  // Restart must keep accepted copies and the review rather than reapplying the empty-test reset.
  await app.restart();
  expect(await facts(app, josiah.id)).toHaveLength(41);
  expect(await facts(app, ansam.id)).toHaveLength(60);
  const ownEvents = await app.request.get(
    `/api/profiles/${josiah.id}/study/events?after=0&limit=100`,
  );
  expect((await ownEvents.json()).items.length).toBeGreaterThan(0);
});
