import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  workers: process.env.CI ? 4 : 3,
  timeout: 45_000,
  globalSetup: "./tests/global-setup.ts",
  reporter: [["list"], ["html", { open: "never" }]],
  expect: {
    timeout: 8_000,
    toHaveScreenshot: { animations: "disabled", maxDiffPixelRatio: 0.005 },
  },
  use: {
    locale: "en-NZ",
    timezoneId: "Pacific/Auckland",
    colorScheme: "light",
    serviceWorkers: "block",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "desktop-chromium",
      use: { browserName: "chromium", viewport: { width: 1440, height: 1000 } },
    },
    {
      name: "mobile-chromium",
      use: { ...devices["Pixel 7"], browserName: "chromium", deviceScaleFactor: 1 },
    },
    {
      name: "mobile-webkit",
      use: { ...devices["iPhone 13"], browserName: "webkit", deviceScaleFactor: 1 },
    },
  ],
});
