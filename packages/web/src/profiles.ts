import type { components } from "@keepsake/client";

// Profile selection belongs to the URL in each tab. Storage is only a root-entry hint.
export type Screen =
  | "library"
  | "practice"
  | "matching"
  | "sharing"
  | "sent"
  | "settings";
export type Profile = components["schemas"]["Profile"];
// Prefilled optional fields for the fact form, from a shortcut or the system share sheet.
export type FactDraft = { context: string; source: string };
export type Share = components["schemas"]["Share"];
export type SharePage = components["schemas"]["SharePage"];

export class RequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

export async function json<T>(url: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(url, {
    ...init,
    headers: { "Content-Type": "application/json", ...init.headers },
  });
  const body: unknown = await response.json();
  if (!response.ok) {
    throw new RequestError(
      typeof body === "object" &&
        body !== null &&
        "error" in body &&
        typeof body.error === "string"
        ? body.error
        : `Request failed (${response.status}).`,
      response.status,
    );
  }
  return body as T;
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Try again.";
}

/** Turn a Web Share Target request into fact-form fields; the learner writes the question. */
export function sharedDraft(params: URLSearchParams): FactDraft {
  const text = params.get("text")?.trim() ?? "";
  // Android browsers often put the shared page's address in the text field.
  const link = /https?:\/\/\S+/.exec(text)?.[0] ?? "";
  const source = (params.get("url")?.trim() || link).slice(0, 2000);
  const parts = [params.get("title")?.trim() ?? "", text.replace(link, "").trim()];
  const context = [...new Set(parts.filter(Boolean))].join("\n\n").slice(0, 30000);
  return { context, source };
}
