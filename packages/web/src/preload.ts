// The first screen's data, requested alongside the profile list so launch waits one round trip, not two.
const paths = {
  library: "facts?limit=25&search=",
  practice: "study/queue?limit=20",
  matching: "facts/sample?limit=4",
};

let launch: { path: string; response: Promise<Response> } | undefined;

export function preload(profileId: number, screen: keyof typeof paths) {
  const path = `/api/profiles/${profileId}/${paths[screen]}`;
  const response = fetch(path);
  // An unused preload must not report an unhandled failure; its consumer sees the rejection.
  response.catch(() => undefined);
  launch = { path, response };
}

/** The preloaded response if this is the identical request. Only the first request is offered it. */
export function preloaded(request: Request): Promise<Response> | undefined {
  const hit = launch;
  launch = undefined;
  const url = new URL(request.url);
  if (hit && request.method === "GET" && url.pathname + url.search === hit.path) {
    return hit.response;
  }
}
