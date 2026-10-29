// Keepsake is meant to be installed. How depends on the browser a visitor is in, so the
// install page reads it from the user agent and shows that browser's steps.

/** Where the browser control to tap first sits, for the arrow that points to it. */
export type Corner = "top-right" | "bottom-right" | "bottom-centre";

export type Guide =
  /** Safari 26 moved Share behind the ⋯ button; earlier versions show it in the toolbar. */
  | { kind: "ios-safari"; shareInMenu: boolean; tablet: boolean }
  /** Other iOS browsers can add to the Home Screen through their own Share button. */
  | { kind: "ios-browser"; browser: "Chrome" | "Firefox" | "Edge" }
  | { kind: "android-chrome" }
  | { kind: "android-browser"; browser: "Samsung Internet" | "Firefox" | "Edge" | "Opera" | null }
  /** Browsers inside other apps cannot install anything; the page must open in the real browser. */
  | { kind: "in-app"; app: string | null; platform: "ios" | "android" }
  | { kind: "desktop"; browser: "chromium" | "safari" | "firefox" | null };

// Apps whose links open in their own browser. Messenger's agent also names Facebook,
// so it comes first.
const inAppBrowsers: [RegExp, string][] = [
  [/Instagram/, "Instagram"],
  [/Messenger|MESSENGER/, "Messenger"],
  [/FBAN|FBAV|FB_IAB|FBIOS/, "Facebook"],
  [/LinkedInApp/, "LinkedIn"],
  [/Snapchat/, "Snapchat"],
  [/musical_ly|TikTok|BytedanceWebview/, "TikTok"],
  [/Pinterest/, "Pinterest"],
  [/\bLine\//, "LINE"],
  [/Twitter/, "X"],
  [/\bGSA\//, "Google"],
];

export function installGuide(userAgent: string, touchPoints: number): Guide {
  // iPadOS reports a Mac user agent; touch support distinguishes it.
  const ios =
    /iPhone|iPad|iPod/.test(userAgent) ||
    (/Macintosh/.test(userAgent) && touchPoints > 1);
  const android = /Android/.test(userAgent);
  const app =
    inAppBrowsers.find(([pattern]) => pattern.test(userAgent))?.[1] ?? null;

  if (ios) {
    if (app) return { kind: "in-app", app, platform: "ios" };
    if (/CriOS\//.test(userAgent)) return { kind: "ios-browser", browser: "Chrome" };
    if (/FxiOS\//.test(userAgent)) return { kind: "ios-browser", browser: "Firefox" };
    if (/EdgiOS\//.test(userAgent)) return { kind: "ios-browser", browser: "Edge" };
    // An app's own web view leaves Safari out of its user agent.
    if (!/Safari\//.test(userAgent)) return { kind: "in-app", app: null, platform: "ios" };
    const version = Number(/Version\/(\d+)/.exec(userAgent)?.[1] ?? 0);
    return {
      kind: "ios-safari",
      shareInMenu: version >= 26,
      tablet: !/iPhone|iPod/.test(userAgent),
    };
  }

  if (android) {
    // "wv" marks an Android web view, which is how apps show links.
    if (app || /; wv\)/.test(userAgent)) return { kind: "in-app", app, platform: "android" };
    if (/SamsungBrowser\//.test(userAgent)) return { kind: "android-browser", browser: "Samsung Internet" };
    if (/Firefox\//.test(userAgent)) return { kind: "android-browser", browser: "Firefox" };
    if (/EdgA\//.test(userAgent)) return { kind: "android-browser", browser: "Edge" };
    if (/OPR\//.test(userAgent)) return { kind: "android-browser", browser: "Opera" };
    if (/Chrome\//.test(userAgent)) return { kind: "android-chrome" };
    return { kind: "android-browser", browser: null };
  }

  if (/Firefox\//.test(userAgent)) return { kind: "desktop", browser: "firefox" };
  if (/Chrome\/|Edg\//.test(userAgent)) return { kind: "desktop", browser: "chromium" };
  if (/Version\/[\d.]+ Safari\//.test(userAgent)) return { kind: "desktop", browser: "safari" };
  return { kind: "desktop", browser: null };
}

/** The corner holding the control to tap first, or null where it varies. */
export function firstTapCorner(guide: Guide): Corner | null {
  switch (guide.kind) {
    case "ios-safari":
      if (guide.tablet) return "top-right";
      return guide.shareInMenu ? "bottom-right" : "bottom-centre";
    case "ios-browser":
      return guide.browser === "Chrome" ? "top-right" : null;
    case "android-chrome":
      return "top-right";
    case "android-browser":
      return guide.browser === "Samsung Internet" ? "bottom-right" : null;
    case "in-app":
      return "top-right";
    case "desktop":
      return null;
  }
}

/**
 * A link that asks the system to open this page in the platform's own browser:
 * Safari on iOS, Chrome on Android.
 */
export function openInBrowserHref(guide: Guide, url: URL): string | null {
  if (guide.kind !== "in-app") return null;
  if (guide.platform === "ios") return `x-safari-${url.href}`;
  return `intent://${url.host}${url.pathname}${url.search}#Intent;scheme=${url.protocol.slice(0, -1)};package=com.android.chrome;end`;
}
