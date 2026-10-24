import { execFile, spawn, type ChildProcess } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { test as base, expect, request, type APIRequestContext } from "@playwright/test";
import type { components } from "@keepsake/client";
import { repoRoot } from "./global-setup";

export type NewFact = components["schemas"]["NewFact"];
export type Fact = components["schemas"]["Fact"];

const run = promisify(execFile);
const binarySuffix = process.platform === "win32" ? ".exe" : "";

function waitForExit(child: ChildProcess, timeout: number): Promise<boolean> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve(true);
  }

  return new Promise((resolve) => {
    const done = () => {
      clearTimeout(timer);
      resolve(true);
    };
    const timer = setTimeout(() => {
      child.off("exit", done);
      resolve(false);
    }, timeout);
    child.once("exit", done);
  });
}

export class TestApp {
  url = "";
  request!: APIRequestContext;
  logs = "";
  readonly databasePath: string;
  private server: ChildProcess | undefined;
  private readonly binaries: string;

  constructor(private readonly directory: string) {
    this.databasePath = path.join(directory, "library.sqlite3");
    const binaries = process.env.KEEPSAKE_E2E_BIN_DIR;
    if (!binaries) {
      throw new Error("The browser suite must run its global setup before starting a test.");
    }
    this.binaries = binaries;
  }

  private environment(): NodeJS.ProcessEnv {
    // An allowlist and a temporary cwd keep shell settings and the checkout's .env out.
    return {
      PATH: process.env.PATH,
      SYSTEMROOT: process.env.SYSTEMROOT,
      DATABASE_URL: `sqlite:${this.databasePath}`,
      WEB_DIR: path.join(repoRoot, "packages/web/dist"),
      BIND_ADDR: "127.0.0.1:0",
      RUST_LOG: "info",
      SHUTDOWN_GRACE_SECS: "2",
    };
  }

  async start(): Promise<void> {
    const child = spawn(path.join(this.binaries, `keepsake-server${binarySuffix}`), [], {
      cwd: this.directory,
      env: this.environment(),
      stdio: ["ignore", "pipe", "pipe"],
    });
    this.server = child;

    try {
      this.url = await new Promise<string>((resolve, reject) => {
        let output = "";
        const timer = setTimeout(() => {
          reject(new Error(`Server did not start within 10 seconds.\n${this.logs}`));
        }, 10_000);

        const collect = (chunk: Buffer) => {
          const text = chunk.toString();
          this.logs = (this.logs + text).slice(-64 * 1024);
          output = (output + text).slice(-64 * 1024);
          const address = /keepsake is listening address=(127\.0\.0\.1:\d+)/.exec(output);
          if (address) {
            clearTimeout(timer);
            resolve(`http://${address[1]}`);
          }
        };
        child.stdout!.on("data", collect);
        child.stderr!.on("data", collect);
        child.once("error", (error) => {
          clearTimeout(timer);
          reject(error);
        });
        child.once("exit", (code, signal) => {
          clearTimeout(timer);
          reject(new Error(`Server exited during startup (${code ?? signal}).\n${this.logs}`));
        });
      });

      this.request = await request.newContext({ baseURL: this.url, timeout: 8_000 });
      const health = await this.request.get("/api/health");
      expect(health.ok(), "the test server must have a healthy migrated database").toBe(true);
    } catch (error) {
      await this.stop();
      throw error;
    }
  }

  async createFact(input: Partial<NewFact> = {}): Promise<Fact> {
    const response = await this.request.post("/api/facts", {
      data: {
        prompt: "What is the synthetic test fact?",
        answer: "A fact created only for this browser test.",
        context: "Synthetic browser fixture.",
        source: "",
        tags: [],
        practices: [],
        ...input,
      } satisfies NewFact,
    });
    expect(response.status(), await response.text()).toBe(201);
    return response.json() as Promise<Fact>;
  }

  async cli(args: string[]): Promise<string> {
    const result = await run(
      path.join(this.binaries, `keepsake${binarySuffix}`),
      ["--database", `sqlite:${this.databasePath}`, ...args],
      { cwd: this.directory, env: this.environment(), timeout: 10_000 },
    );
    return result.stdout;
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const child = this.server;
    this.server = undefined;
    try {
      await this.request?.dispose();
    } finally {
      // Request-context teardown must not prevent the owned process from being stopped.
      if (child) {
        child.kill("SIGTERM");
        if (!(await waitForExit(child, 4_000))) {
          child.kill("SIGKILL");
          if (!(await waitForExit(child, 2_000))) {
            throw new Error("The browser test's server could not be stopped.");
          }
        }
      }
    }
  }

  async dispose(): Promise<void> {
    await this.stop();
    await rm(this.directory, { recursive: true, force: true });
  }
}

export const test = base.extend<{ app: TestApp }>({
  app: async ({}, use, testInfo) => {
    const app = new TestApp(await mkdtemp(path.join(os.tmpdir(), "keepsake-browser-")));
    try {
      await app.start();
      await use(app);
    } finally {
      if (testInfo.status !== testInfo.expectedStatus) {
        await testInfo.attach("server.log", { body: app.logs, contentType: "text/plain" });
      }
      await app.dispose();
    }
  },
  baseURL: async ({ app }, use) => {
    await use(app.url);
  },
  context: async ({ context, app }, use) => {
    const errors: string[] = [];
    context.on("page", (page) => page.on("pageerror", (error) => errors.push(error.message)));
    await context.route("**/*", async (route) => {
      const url = new URL(route.request().url());
      if (url.origin !== app.url) {
        errors.push(`Unexpected browser request outside the test server: ${url.origin}`);
        await route.abort("blockedbyclient");
        return;
      }
      await route.continue();
    });

    await use(context);
    expect(errors, "browser exceptions and external requests must fail the test").toEqual([]);
  },
});

export { expect };
