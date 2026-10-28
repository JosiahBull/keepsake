import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures";

type Rpc = { result?: { tools?: { name: string }[] }; error?: { message: string } };

async function openSettings(page: Page) {
  await page.goto("/p/1/settings");
  return page.getByRole("region", { name: "Agent access", exact: true });
}

test.describe("with KEEPSAKE_MCP=write", () => {
  test.use({ serverSettings: { KEEPSAKE_MCP: "write" } });

  test("a profile turns agent access on and off, and the endpoint follows", async ({ page, app }) => {
    const tools = async () => {
      const response = await app.request.post("/mcp/default", {
        headers: { accept: "application/json, text/event-stream", "mcp-protocol-version": "2025-06-18" },
        data: { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} },
      });
      expect(response.status(), await response.text()).toBe(200);
      return ((await response.json()) as Rpc).result?.tools?.map((tool) => tool.name).sort() ?? [];
    };

    const section = await openSettings(page);
    const access = section.getByRole("combobox", { name: "Access", exact: true });
    await expect(access).toHaveValue("off");
    await expect(section.getByRole("option")).toHaveText(["Off", "Read only", "Read and add facts"]);
    await expect(section.getByText("claude mcp add", { exact: false })).toHaveCount(0);
    expect(await tools()).toEqual([]);

    await access.selectOption("read");
    await expect(section.getByRole("status")).toHaveText("Agent access: read only.");
    await expect(section.locator("pre")).toHaveText(
      `claude mcp add --transport http keepsake-default ${app.url}/mcp/default`,
    );
    expect(await tools()).toEqual(["get_fact", "list_tags", "search_facts", "study_overview", "view_media"]);

    await access.selectOption("write");
    await expect(section.getByRole("status")).toHaveText("Agent access: read and add facts.");
    expect(await tools()).toContain("add_facts");

    await access.selectOption("off");
    await expect(section.getByRole("status")).toHaveText("Agent access: off.");
    expect(await tools()).toEqual([]);

    await page.reload();
    await expect(section.getByRole("combobox", { name: "Access", exact: true })).toHaveValue("off");
  });
});

test.describe("with KEEPSAKE_MCP=read", () => {
  test.use({ serverSettings: { KEEPSAKE_MCP: "read" } });

  test("the app offers no more than the server allows", async ({ page }) => {
    const section = await openSettings(page);
    await expect(section.getByRole("option")).toHaveText(["Off", "Read only"]);
    await expect(section.getByText("This server allows read-only access at most.")).toBeVisible();
  });
});

test("without KEEPSAKE_MCP the setting explains why it is unavailable", async ({ page, app }) => {
  const section = await openSettings(page);
  await expect(section.getByText(/Not available on this server/)).toBeVisible();
  await expect(section.getByRole("combobox")).toHaveCount(0);
  const response = await app.request.post("/mcp/default", {
    headers: { accept: "application/json, text/event-stream" },
    data: { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} },
  });
  expect(response.status()).not.toBe(200);
});
