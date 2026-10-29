import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures";

// Chromium may offer its own install prompt in tests. The page never hears of it, so each
// case decides whether there is a prompt.
async function withoutInstallPrompt(page: Page) {
  await page.addInitScript(() => {
    addEventListener("beforeinstallprompt", (event) => {
      if (event.isTrusted) event.stopImmediatePropagation();
    }, { capture: true });
  });
}

/** Offers a stand-in for the browser's install prompt, recording whether it was shown. */
async function offerInstallPrompt(page: Page) {
  await page.evaluate(() => {
    const event = new Event("beforeinstallprompt", { cancelable: true });
    Object.assign(event, {
      prompt: async () => {
        Object.assign(window, { keepsakePrompted: true });
      },
      userChoice: Promise.resolve({ outcome: "accepted" }),
    });
    dispatchEvent(event);
  });
}

test("the front page shows this device's way to install, and can still open the app in the browser", async ({
  page,
  isMobile,
  browserName,
}) => {
  await withoutInstallPrompt(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Keep what you learn", level: 1 })).toBeVisible();

  if (!isMobile) {
    // Desktops are pointed at a phone, and at the browser's own install where there is one.
    await expect(page.getByRole("img", { name: /^QR code for http:\/\/127\.0\.0\.1:\d+\/$/ })).toBeVisible();
    await expect(page.getByText("Scan with your phone’s camera to install Keepsake there.")).toBeVisible();
    await expect(page.getByText(/Chrome and Edge can also install Keepsake/)).toBeVisible();
  } else {
    await expect(page.getByRole("img", { name: /^QR code/ })).toHaveCount(0);
    const install = page.getByRole("button", { name: "Install Keepsake", exact: true });
    // From the keyboard, so every engine has focus to return to the button.
    await install.focus();
    await page.keyboard.press("Enter");
    const steps = page.getByRole("dialog");
    if (browserName === "webkit") {
      await expect(steps.getByRole("heading", { name: "Add Keepsake to your Home Screen" })).toBeVisible();
      await expect(steps.getByRole("listitem")).toHaveText([
        "Tap at the bottom of the screen, then Share",
        "Scroll down and choose Add to Home Screen",
        "Leave Open as Web App on, then tap Add",
      ]);
      await expect(steps.getByRole("img", { name: "More", exact: true })).toBeVisible();
    } else {
      await expect(steps.getByRole("heading", { name: "Install Keepsake" })).toBeVisible();
      await expect(steps.getByRole("listitem")).toHaveText([
        "Tap at the top right",
        "Choose Install app or Add to Home screen",
        "Choose Install",
      ]);
    }
    await page.keyboard.press("Escape");
    await expect(steps).toHaveCount(0);
    await expect(install).toBeFocused();
  }

  await page.getByRole("link", { name: "Use it in this browser instead" }).first().click();
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("heading", { name: "Library", level: 1 })).toBeVisible();
});

test("where the browser offers its own install prompt, one tap installs", async ({ page, isMobile, browserName }) => {
  test.skip(browserName !== "chromium", "Only Chromium browsers offer the prompt.");
  await withoutInstallPrompt(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Keep what you learn", level: 1 })).toBeVisible();
  await offerInstallPrompt(page);
  await page.getByRole("button", { name: isMobile ? "Install Keepsake" : "Install on this computer", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText(
    `Keepsake is installed. Open it from your ${isMobile ? "Home Screen" : "apps"}.`,
  );
  expect(await page.evaluate(() => Reflect.get(window, "keepsakePrompted"))).toBe(true);
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("the installed app skips the front page", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "standalone", { configurable: true, get: () => true });
  });
  await page.goto("/");
  await expect(page).toHaveURL(/\/p\/1\/library$/);
  await expect(page.getByRole("heading", { name: "Library", level: 1 })).toBeVisible();
});

// The steps follow the browser, so these cases borrow other browsers' user agents.
const iPhone = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko)";
const android = "Mozilla/5.0 (Linux; Android 14; Pixel 7)";
for (const [name, userAgent, heading, steps, hint] of [
  [
    "Safari before version 26",
    `${iPhone} Version/18.0 Mobile/15E148 Safari/604.1`,
    "Add Keepsake to your Home Screen",
    ["Tap Share at the bottom of the screen", "Scroll down and choose Add to Home Screen", "Tap Add"],
    null,
  ],
  [
    "Chrome on iPhone",
    `${iPhone} CriOS/130.0.6723.90 Mobile/15E148 Safari/604.1`,
    "Add Keepsake to your Home Screen",
    ["Tap Share at the end of the address bar", "Choose Add to Home Screen", "Tap Add"],
    "No Add to Home Screen? Open this page in Safari and add it from there.",
  ],
  [
    "Samsung Internet",
    `${android} AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/26.0 Chrome/122.0.0.0 Mobile Safari/537.36`,
    "Install Keepsake",
    ["Tap at the bottom right", "Choose Add page to, then Home screen", "Tap Add"],
    null,
  ],
  [
    "Firefox on Android",
    `${android}; Mobile; rv:131.0) Gecko/131.0 Firefox/131.0`,
    "Install Keepsake",
    ["Tap", "Choose Add app to Home screen", "Tap Add"],
    null,
  ],
] as const) {
  test.describe(name, () => {
    test.use({ userAgent, hasTouch: true });
    test(`${name} gets its own install steps`, async ({ page }, testInfo) => {
      test.skip(testInfo.project.name !== "desktop-chromium", "The user agent decides; one engine is enough.");
      await withoutInstallPrompt(page);
      await page.goto("/");
      await page.getByRole("button", { name: "Install Keepsake", exact: true }).click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("heading", { name: heading })).toBeVisible();
      await expect(dialog.getByRole("listitem")).toHaveText([...steps]);
      if (hint) await expect(dialog.getByText(hint)).toBeVisible();
    });
  });
}

test.describe("an app's own browser", () => {
  test.use({ userAgent: `${iPhone} Mobile/15E148 Instagram 350.0.0.0`, hasTouch: true });
  test("is asked to open the page in Safari, or to copy its link", async ({ app, page, context }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop-chromium", "The user agent decides; one engine is enough.");
    await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: app.url });
    await page.goto("/");
    await page.getByRole("button", { name: "Install Keepsake", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "Instagram’s browser can’t install Keepsake" })).toBeVisible();
    await expect(dialog.getByRole("link", { name: "Open in Safari" })).toHaveAttribute("href", `x-safari-${app.url}/`);
    await dialog.getByRole("button", { name: "Copy link", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Link copied", exact: true })).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(`${app.url}/`);
  });
});

for (const [name, userAgent, note] of [
  [
    "Safari on a Mac",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
    "On a Mac, Safari can add Keepsake to the Dock: choose File, then Add to Dock.",
  ],
  [
    "Firefox on a computer",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 15.0; rv:131.0) Gecko/20100101 Firefox/131.0",
    null,
  ],
] as const) {
  test.describe(name, () => {
    test.use({ userAgent });
    test(`${name} is shown the QR code${note ? " and its own install" : ""}`, async ({ page }, testInfo) => {
      test.skip(testInfo.project.name !== "desktop-chromium", "The user agent decides; one engine is enough.");
      await withoutInstallPrompt(page);
      await page.goto("/");
      await expect(page.getByRole("img", { name: /^QR code for / })).toBeVisible();
      if (note) await expect(page.getByText(note)).toBeVisible();
      else await expect(page.getByText(/on this computer|Add to Dock/)).toHaveCount(0);
      await expect(page.getByRole("button", { name: "Install Keepsake" })).toHaveCount(0);
    });
  });
}
