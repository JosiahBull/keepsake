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
