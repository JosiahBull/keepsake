import { test, expect, type Fact } from "./fixtures";
import { startExtraPractice } from "./actions";
import { png, wav } from "./media-files";

type FactPage = { items: Fact[]; total: number };

test("a question keeps its image and clip from the form through to practice", async ({ page, app }) => {
  const description = "A large green and white pigeon";
  await page.goto("/app");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByRole("textbox", { name: "Question", exact: true }).fill("Which bird is this?");
  await page.getByRole("textbox", { name: "Answer", exact: true }).fill("Kererū");
  await page.getByLabel("Images and audio").setInputFiles([
    { name: "kereru.png", mimeType: "image/png", buffer: png(24, 16) },
    { name: "call.wav", mimeType: "audio/wav", buffer: wav(300) },
  ]);
  await page.getByRole("textbox", { name: /^Describe the image/ }).fill(description);
  await page.getByRole("button", { name: "Save fact", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Fact saved." })).toBeVisible();

  const library = (await (await app.request.get("/api/profiles/1/facts")).json()) as FactPage;
  const [image, clip] = library.items[0].media;
  // Served as AVIF and AAC in MP4, whatever was uploaded.
  expect(image).toMatchObject({ kind: "image", content_type: "image/avif", width: 24, height: 16, alt: description });
  expect(clip).toMatchObject({ kind: "audio", content_type: "audio/mp4", duration_ms: 300, alt: "" });

  const row = page.getByRole("article").filter({ hasText: "Which bird is this?" });
  await expect(row.getByText("1 image, 1 audio clip", { exact: true })).toBeVisible();
  await row.getByRole("heading", { name: "Which bird is this?", exact: true }).click();
  // The browser itself decodes the AVIF and reads the clip's length from its index.
  const shown = row.getByRole("img", { name: description, exact: true });
  await expect(shown).toBeVisible();
  await expect.poll(() => shown.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(24);
  const player = row.getByLabel("Question audio", { exact: true });
  await expect(player).toBeVisible();
  await expect
    .poll(() => player.evaluate((element: HTMLAudioElement) => element.duration))
    .toBeGreaterThan(0.25);
  expect(await player.evaluate((element: HTMLAudioElement) => element.duration)).toBeLessThan(0.5);

  // Safari plays audio only from servers that answer byte ranges.
  const range = await app.request.get(`/api/profiles/1/media/${clip.media_id}`, {
    headers: { range: "bytes=4-11" },
  });
  expect(range.status()).toBe(206);
  expect(await range.body()).toEqual(Buffer.from("ftypM4A "));

  await startExtraPractice(page);
  const stage = page.getByRole("region", { name: "Flashcards", exact: true });
  await expect(stage.getByRole("heading", { name: "Which bird is this?", exact: true })).toBeVisible();
  await expect(stage.getByRole("img", { name: description, exact: true })).toBeVisible();
  await expect(stage.getByLabel("Question audio", { exact: true })).toBeVisible();
});

test("a file that is not what it claims to be is refused and nothing is saved", async ({ page, app }) => {
  await page.goto("/app");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByRole("textbox", { name: "Question", exact: true }).fill("Which bird is this?");
  await page.getByRole("textbox", { name: "Answer", exact: true }).fill("Tūī");
  await page.getByLabel("Images and audio").setInputFiles({
    name: "photo.png",
    mimeType: "image/png",
    buffer: Buffer.from("<html><script>alert(1)</script></html>"),
  });
  await page.getByRole("button", { name: "Save fact", exact: true }).click();

  await expect(page.getByRole("alert")).toHaveText(
    "Could not save the fact. photo.png: The file is not a PNG image. Its contents do not match its type.",
  );
  await expect(page.getByRole("button", { name: "Remove photo.png", exact: true })).toBeVisible();
  const library = (await (await app.request.get("/api/profiles/1/facts")).json()) as FactPage;
  expect(library.total).toBe(0);
});

test("a GIF is turned away before it is uploaded", async ({ page }) => {
  const uploads: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && new URL(request.url()).pathname.endsWith("/media")) {
      uploads.push(request.url());
    }
  });
  await page.goto("/app");
  await page.getByRole("button", { name: "Add a fact", exact: true }).click();
  await page.getByLabel("Images and audio").setInputFiles({
    name: "dance.gif",
    mimeType: "image/gif",
    buffer: Buffer.from("GIF89a\x01\x00\x01\x00\x00\x00\x00;", "binary"),
  });
  await expect(page.getByRole("alert")).toHaveText(
    "dance.gif is not a supported file. Use a JPEG, PNG or WebP image, or an MP3, M4A, AAC, WAV, Ogg, WebM or FLAC clip.",
  );
  await expect(page.getByRole("button", { name: "Remove dance.gif", exact: true })).toHaveCount(0);
  expect(uploads).toEqual([]);
});

test("matching shows each question's picture, so identical wording stays distinct", async ({ page, app }) => {
  for (const [index, bird] of ["Kererū", "Tūī", "Pīwakawaka"].entries()) {
    const upload = await app.request.post("/api/profiles/1/media", {
      headers: { "content-type": "image/png" },
      data: png(12 + index, 12),
    });
    expect(upload.status(), await upload.text()).toBe(201);
    const { media_id } = (await upload.json()) as { media_id: number };
    await app.createFact({
      prompt: "What is this bird?",
      answer: bird,
      tags: ["birds"],
      media: [{ media_id, alt: `Bird photo ${index + 1}` }],
    });
  }

  await page.goto("/p/1/matching");
  const questions = page.getByRole("region", { name: "Matching pairs game" }).locator(".matching-columns > div").first();
  for (const index of [1, 2, 3]) {
    await expect(questions.getByRole("button", { name: new RegExp(`Bird photo ${index}`) })).toBeVisible();
  }
});
