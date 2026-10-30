import type { Screen } from "./profiles";

// Each screen's first request, made before the screen opens so it opens without waiting.
const paths = {
  library: "facts?limit=25&search=",
  practice: "study/queue?limit=20",
  matching: "facts/sample?limit=4",
  sharing: "shares?direction=inbox&limit=25",
  sent: "shares?direction=sent&limit=25",
} satisfies Partial<Record<Screen, string>>;

export type PreloadedScreen = keyof typeof paths;
export const preloadedScreens = Object.keys(paths) as PreloadedScreen[];

// Older than this, another device or household member may have changed what a preload shows.
const MAX_AGE_MS = 60_000;
// Practice and matching records change which questions are due, and nothing else listed here.
const STUDY_RECORD = /\/(reviews|study\/events|study\/exposures)$/;

const preloads = new Map<string, { response: Promise<Response>; at: number }>();
// index.html asks for the profile list before this code has loaded.
const profiles = (window as { keepsakeProfiles?: Promise<Response> })
  .keepsakeProfiles;
if (profiles)
  preloads.set("/api/profiles", { response: profiles, at: Date.now() });

/** Request a screen's data now, unless a recent request for it is still unused. */
export function preload(profileId: number, screen: PreloadedScreen) {
  const path = `/api/profiles/${profileId}/${paths[screen]}`;
  const held = preloads.get(path);
  if (held && Date.now() - held.at < MAX_AGE_MS) return;
  const response = fetch(path);
  // An unused preload must not report an unhandled failure.
  response.catch(() => undefined);
  preloads.set(path, { response, at: Date.now() });
}

/** Fetch, answering a GET from its preload. Only the first identical request is offered it. */
export function fetchPreloaded(request: Request): Promise<Response> {
  const url = new URL(request.url);
  if (request.method !== "GET") {
    // A preload requested before this write lands may not show it.
    discard(url.pathname);
    return fetch(request).finally(() => discard(url.pathname));
  }
  const path = url.pathname + url.search;
  const held = preloads.get(path);
  preloads.delete(path);
  if (!held || Date.now() - held.at >= MAX_AGE_MS) return fetch(request);
  // A preload that failed, perhaps while offline, says nothing about a request made now.
  return held.response.catch(() => fetch(request));
}

function discard(written: string) {
  const study = STUDY_RECORD.test(written);
  for (const path of preloads.keys())
    if (!study || path.endsWith(`/${paths.practice}`)) preloads.delete(path);
}
