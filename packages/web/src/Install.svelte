<script lang="ts">
  import { onMount } from "svelte";
  import { encode } from "uqr";
  import { openDialog } from "./dialog";
  import {
    firstTapCorner,
    installGuide,
    openInBrowserHref,
  } from "./install";
  import { serviceWorker } from "./pwa";

  // Chromium browsers offer their own install prompt once the page is installable.
  type InstallPrompt = Event & {
    prompt(): Promise<void>;
    userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
  };

  const guide = installGuide(navigator.userAgent, navigator.maxTouchPoints);
  const corner = firstTapCorner(guide);
  const home = new URL("/", location.origin);
  const openElsewhere = openInBrowserHref(guide, home);
  const desktop = guide.kind === "desktop";
  const qr = desktop ? encode(home.href, { border: 0 }) : null;
  const qrPath = qr
    ? qr.data
        .flatMap((row, y) =>
          row.map((dark, x) => (dark ? `M${x},${y}h1v1h-1z` : "")),
        )
        .join("")
    : "";

  let prompt = $state<InstallPrompt | null>(null);
  let installed = $state(false);
  let showSteps = $state(false);
  let copied = $state(false);

  onMount(() => {
    // Registering the worker lets browsers treat Keepsake as installable from this page.
    void serviceWorker();
    const offer = (event: Event) => {
      event.preventDefault();
      prompt = event as InstallPrompt;
    };
    const done = () => {
      installed = true;
      prompt = null;
      showSteps = false;
    };
    addEventListener("beforeinstallprompt", offer);
    addEventListener("appinstalled", done);
    return () => {
      removeEventListener("beforeinstallprompt", offer);
      removeEventListener("appinstalled", done);
    };
  });

  async function install() {
    const offered = prompt;
    if (!offered) {
      showSteps = true;
      return;
    }
    prompt = null;
    await offered.prompt();
    if ((await offered.userChoice).outcome === "accepted") installed = true;
  }

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(home.href);
      copied = true;
    } catch {
      // The address stays on screen to copy by hand.
      copied = false;
    }
  }
</script>

<svelte:head><title>Install Keepsake</title></svelte:head>

{#snippet icon(name: "share" | "more" | "menu" | "lines", label: string | null)}
  <span
    class="chip"
    role={label ? "img" : undefined}
    aria-label={label ?? undefined}
    aria-hidden={label ? undefined : "true"}
  >
    <svg viewBox="0 0 24 24" width="22" height="22">
      {#if name === "share"}
        <path d="M12 3.5v11M8 7.5l4-4 4 4" />
        <path d="M8.5 10.5H6.5v10h11v-10h-2" />
      {:else if name === "more"}
        <circle cx="6" cy="12" r="1.4" class="dot" />
        <circle cx="12" cy="12" r="1.4" class="dot" />
        <circle cx="18" cy="12" r="1.4" class="dot" />
      {:else if name === "menu"}
        <circle cx="12" cy="6" r="1.4" class="dot" />
        <circle cx="12" cy="12" r="1.4" class="dot" />
        <circle cx="12" cy="18" r="1.4" class="dot" />
      {:else}
        <path d="M5 7h14M5 12h14M5 17h14" />
      {/if}
    </svg>
  </span>
{/snippet}

{#snippet preview()}
  <!-- A glimpse of practice, drawn rather than captured so it stays crisp. -->
  <div class="preview" aria-hidden="true">
    <div class="preview-bar">Practise</div>
    <div class="preview-card">
      <p class="preview-meta">Flashcards · 3 of 12</p>
      <p class="preview-question">What does wai mean?</p>
      <p class="preview-answer">Water</p>
      <div class="preview-ratings">
        <span>Again</span><span>Hard</span><span class="chosen">Good</span><span
          >Easy</span
        >
      </div>
    </div>
  </div>
{/snippet}

<main class="install" class:desktop>
  <img
    class="logo"
    src="/keepsake-logo.svg"
    alt="Keepsake"
    width="2172"
    height="724"
  />

  <div class="layout">
    <div class="pitch">
      <h1>Keep what you learn</h1>
      <p class="lede">
        Keepsake is a personal flashcard library. Save the facts you want to
        remember, and it brings each one back for practice when it is due.
      </p>

      {#if installed}
        <p class="installed" role="status">
          Keepsake is installed. Open it from your {desktop
            ? "apps"
            : "Home Screen"}.
        </p>
      {:else if desktop}
        <figure class="qr">
          <svg
            role="img"
            aria-label={`QR code for ${home.href}`}
            viewBox={`-2 -2 ${(qr?.size ?? 0) + 4} ${(qr?.size ?? 0) + 4}`}
            width="132"
            height="132"
            shape-rendering="crispEdges"><path d={qrPath} /></svg
          >
          <figcaption>
            Scan with your phone’s camera to install Keepsake there.
          </figcaption>
        </figure>
        {#if prompt}
          <button class="secondary" onclick={install}
            >Install on this computer</button
          >
        {:else if guide.kind === "desktop" && guide.browser === "chromium"}
          <p class="note">
            Chrome and Edge can also install Keepsake on this computer, from
            the install icon at the end of the address bar.
          </p>
        {:else if guide.kind === "desktop" && guide.browser === "safari"}
          <p class="note">
            On a Mac, Safari can add Keepsake to the Dock: choose File, then
            Add to Dock.
          </p>
        {/if}
      {:else}
        <button class="primary" onclick={install}>Install Keepsake</button>
        <p class="note">
          Installed, Keepsake opens full screen and can remind you when
          practice is due.
        </p>
      {/if}

      <a class="browser" href="/app">Use it in this browser instead</a>
    </div>

    {@render preview()}
  </div>
</main>

{#if showSteps}
  <dialog
    class="steps"
    use:openDialog
    onclose={() => (showSteps = false)}
    aria-labelledby="steps-title"
  >
    <button class="close" aria-label="Close" onclick={() => (showSteps = false)}
      >×</button
    >
    {#if corner}
      <svg class={`arrow ${corner}`} viewBox="0 0 60 120" aria-hidden="true">
        <path d="M30 116C12 92 48 74 30 50S30 18 30 6M18 19 30 5l12 14" />
      </svg>
    {/if}

    <div class="steps-body">
      {#if guide.kind === "in-app"}
        <h2 id="steps-title">
          {guide.app ? `${guide.app}’s` : "This app’s"} browser can’t install Keepsake
        </h2>
        <p>
          Open this page in {guide.platform === "ios" ? "Safari" : "Chrome"} first,
          then follow the steps there.
        </p>
        <div class="steps-actions">
          {#if openElsewhere}<a class="light" href={openElsewhere}
              >Open in {guide.platform === "ios" ? "Safari" : "Chrome"}</a
            >{/if}
          <button class="outline" onclick={copyLink}
            >{copied ? "Link copied" : "Copy link"}</button
          >
        </div>
        <p class="hint">
          Or tap {@render icon("more", "More")} in the corner and choose to open
          the page in your browser.
        </p>
        <p class="address">{home.href}</p>
      {:else}
        <h2 id="steps-title">
          {guide.kind === "ios-safari" || guide.kind === "ios-browser"
            ? "Add Keepsake to your Home Screen"
            : "Install Keepsake"}
        </h2>
        <ol>
          {#if guide.kind === "ios-safari"}
            {#if guide.tablet}
              <li>Tap {@render icon("share", null)} Share at the top of the screen</li>
            {:else if guide.shareInMenu}
              <li>
                Tap {@render icon("more", "More")} at the bottom of the screen, then
                {@render icon("share", null)} Share
              </li>
            {:else}
              <li>Tap {@render icon("share", null)} Share at the bottom of the screen</li>
            {/if}
            <li>Scroll down and choose Add to Home Screen</li>
            <li>
              {guide.shareInMenu
                ? "Leave Open as Web App on, then tap Add"
                : "Tap Add"}
            </li>
          {:else if guide.kind === "ios-browser"}
            {#if guide.browser === "Chrome"}
              <li>Tap {@render icon("share", null)} Share at the end of the address bar</li>
            {:else}
              <li>
                Open the {guide.browser} menu, then tap {@render icon("share", null)} Share
              </li>
            {/if}
            <li>Choose Add to Home Screen</li>
            <li>Tap Add</li>
          {:else if guide.kind === "android-chrome"}
            <li>Tap {@render icon("menu", "Menu")} at the top right</li>
            <li>Choose Install app or Add to Home screen</li>
            <li>Choose Install</li>
          {:else if guide.kind === "android-browser" && guide.browser === "Samsung Internet"}
            <li>Tap {@render icon("lines", "Menu")} at the bottom right</li>
            <li>Choose Add page to, then Home screen</li>
            <li>Tap Add</li>
          {:else if guide.kind === "android-browser" && guide.browser === "Firefox"}
            <li>Tap {@render icon("menu", "Menu")}</li>
            <li>Choose Add app to Home screen</li>
            <li>Tap Add</li>
          {:else}
            <li>Open your browser’s menu</li>
            <li>Choose Install app or Add to Home screen</li>
            <li>Confirm</li>
          {/if}
        </ol>
        {#if guide.kind === "ios-safari" && guide.shareInMenu}
          <p class="hint">If Share is already on the toolbar, tap it there.</p>
        {:else if guide.kind === "ios-browser"}
          <p class="hint">
            No Add to Home Screen? Open this page in Safari and add it from
            there.
          </p>
        {:else if guide.kind === "android-chrome"}
          <p class="hint">
            Choose Install rather than Create shortcut, which opens Keepsake in
            Chrome. Address bar at the bottom? The menu is there instead.
          </p>
        {/if}
      {/if}
      <a class="browser" href="/app">Use it in this browser instead</a>
    </div>
  </dialog>
{/if}

<style>
  .install {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    max-width: 1100px;
    margin: auto;
    padding: 1.5rem 1.5rem 2.5rem;
  }

  .logo {
    width: 140px;
    height: auto;
    align-self: center;
  }

  .layout {
    flex: 1;
    display: flex;
    flex-direction: column-reverse;
    justify-content: center;
    align-items: center;
    gap: 2rem;
    padding-top: 1.5rem;
  }

  .pitch {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 30rem;
  }

  h1 {
    font:
      500 2.4rem/1.1 "Iowan Old Style",
      "Palatino Linotype",
      Palatino,
      serif;
    letter-spacing: -0.035em;
    margin: 0 0 0.9rem;
  }

  .lede {
    color: #52685d;
    font-size: 1.05rem;
    margin-bottom: 1.6rem;
  }

  .primary {
    width: min(100%, 20rem);
    padding: 1rem 1.5rem;
    border-radius: 999px;
    background: #244c44;
    color: #fff;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .primary:hover,
  .secondary:hover {
    background: #183c34;
  }

  .secondary {
    padding: 0.75rem 1.2rem;
    border-radius: 999px;
    background: #244c44;
    color: #fff;
    font-weight: 600;
    margin-bottom: 1rem;
  }

  .note {
    color: #64786c;
    font-size: 0.88rem;
    margin: 1rem 0 0;
  }

  .browser {
    margin-top: 1.2rem;
    color: #365c6a;
    font-size: 0.95rem;
  }

  .installed {
    background: #e3ede7;
    padding: 1rem 1.2rem;
    border-radius: 8px;
  }

  .qr {
    display: flex;
    align-items: center;
    gap: 1.2rem;
    margin: 0 0 1.2rem;
    padding: 1rem;
    background: #fff;
    border: 1px solid #c6d7cd;
    border-radius: 14px;
    text-align: left;
  }

  .qr svg {
    flex-shrink: 0;
    fill: #18362f;
  }

  .qr figcaption {
    max-width: 12rem;
    font-weight: 600;
    font-size: 0.95rem;
  }

  /* The preview: a phone-shaped card mid-practice. */
  .preview {
    width: 230px;
    padding: 0.75rem 0.75rem 1rem;
    background: #fff;
    border: 1px solid #c6d7cd;
    border-radius: 28px;
    box-shadow: 0 18px 40px #18362f1f;
    transform: rotate(-3deg);
  }

  .preview-bar {
    text-align: center;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.2rem 0 0.6rem;
    border-bottom: 0.5px solid #18362f26;
  }

  .preview-card {
    padding: 0.8rem 0.4rem 0;
  }

  .preview p {
    margin: 0;
    line-height: 1.4;
  }

  .preview-meta {
    color: #64786c;
    font-size: 0.62rem;
  }

  .preview .preview-question {
    font:
      500 1.05rem/1.3 "Iowan Old Style",
      Palatino,
      serif;
    margin: 0.4rem 0 0.7rem;
  }

  .preview-answer {
    font-size: 0.8rem;
    padding: 0.5rem 0.6rem;
    background: #f0f5f1;
    border-left: 2px solid #618573;
    border-radius: 4px;
  }

  .preview-ratings {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.25rem;
    margin-top: 0.7rem;
  }

  .preview-ratings span {
    padding: 0.35rem 0;
    border: 1px solid #c6d7cd;
    border-radius: 5px;
    font-size: 0.58rem;
    font-weight: 600;
    text-align: center;
    color: #244c44;
  }

  .preview-ratings .chosen {
    background: #244c44;
    border-color: #244c44;
    color: #fff;
  }

  /* Desktops put the pitch beside the preview. */
  .desktop .logo {
    align-self: flex-start;
  }

  @media (min-width: 820px) {
    .desktop .layout {
      flex-direction: row;
      justify-content: space-between;
      gap: 4rem;
    }

    .desktop .pitch {
      align-items: flex-start;
      text-align: left;
    }

    .desktop h1 {
      font-size: 3.2rem;
    }

    .desktop .preview {
      width: 300px;
      margin-right: 3rem;
    }
  }

  @media (max-width: 380px) {
    h1 {
      font-size: 2rem;
    }

    .preview {
      width: 200px;
    }
  }

  /* Steps cover the page, leaving the browser's own controls in view to point at. */
  .steps {
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: 0;
    border: 0;
    background: #18362ff2;
    color: #fff;
  }

  .steps::backdrop {
    background: transparent;
  }

  .steps-body {
    min-height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    max-width: 28rem;
    margin: auto;
    padding: 5rem 1.5rem 6rem;
    text-align: center;
  }

  .steps h2 {
    font:
      500 1.8rem/1.2 "Iowan Old Style",
      Palatino,
      serif;
    margin: 0 0 1.5rem;
  }

  .steps ol {
    list-style: none;
    counter-reset: step;
    margin: 0 0 1.5rem;
    padding: 0;
    display: grid;
    gap: 1.1rem;
  }

  .steps li {
    counter-increment: step;
    font-size: 1.2rem;
    font-weight: 600;
    line-height: 1.6;
  }

  .steps li::before {
    content: counter(step);
    display: inline-grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    margin-right: 0.5rem;
    border: 1.5px solid #ffffff80;
    border-radius: 50%;
    font-size: 0.85rem;
    vertical-align: 0.15em;
  }

  .steps p {
    margin: 0 0 1rem;
  }

  .hint {
    color: #ffffffb8;
    font-size: 0.95rem;
  }

  .chip {
    display: inline-grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    margin: 0 0.15rem;
    vertical-align: middle;
    background: #fff;
    color: #18362f;
    border-radius: 8px;
  }

  .chip svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .chip .dot {
    fill: currentColor;
    stroke: none;
  }

  .steps-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.7rem;
    margin-bottom: 1.4rem;
  }

  .light,
  .outline {
    padding: 0.85rem 1.4rem;
    border-radius: 999px;
    font-weight: 600;
    text-decoration: none;
  }

  .light {
    background: #fff;
    color: #18362f;
  }

  .outline {
    background: none;
    color: #fff;
    border: 1.5px solid #ffffff99;
  }

  .address {
    font-size: 0.85rem;
    color: #ffffffb8;
    user-select: all;
    overflow-wrap: anywhere;
  }

  .steps .browser {
    color: #fff;
  }

  .close {
    position: absolute;
    top: calc(0.75rem + env(safe-area-inset-top));
    left: 0.75rem;
    width: 44px;
    height: 44px;
    background: none;
    color: #fff;
    font-size: 2rem;
    line-height: 1;
  }

  /* Points from the steps toward the browser control to tap first. */
  .arrow {
    position: absolute;
    width: 48px;
    height: 96px;
    fill: none;
    stroke: #fff;
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
    transform: rotate(var(--turn));
    animation: nudge 1.6s ease-in-out infinite;
  }

  .arrow.top-right {
    top: calc(0.5rem + env(safe-area-inset-top));
    right: 1.5rem;
    --turn: 25deg;
  }

  .arrow.bottom-right {
    bottom: calc(0.5rem + env(safe-area-inset-bottom));
    right: 1.5rem;
    --turn: 155deg;
  }

  .arrow.bottom-centre {
    bottom: calc(0.5rem + env(safe-area-inset-bottom));
    left: calc(50% - 24px);
    --turn: 180deg;
  }

  @keyframes nudge {
    0%,
    100% {
      transform: rotate(var(--turn)) translateY(0);
    }
    50% {
      transform: rotate(var(--turn)) translateY(-6px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .arrow {
      animation: none;
    }
  }
</style>
