<script lang="ts">
  import type { Snippet } from "svelte";

  type View = "library" | "practice" | "matching";

  // Every screen shares this header, so switching screens never moves it.
  // Phones get an iOS-style navigation bar with the title, and a tab bar at the bottom.
  let {
    profileId,
    title,
    detail = null,
    back = null,
    current,
    disabled,
    adding = false,
    onNavigate,
    onAdd,
    profileMenu,
    children,
  }: {
    profileId: number;
    title: string;
    /** A line under the title, such as the fact count. An empty string keeps the
     * line while it loads, so the title stays put; null has no line. */
    detail?: string | null;
    /** A screen pushed on top of another, such as settings, can go back to it. */
    back?: { label: string; onBack: () => void } | null;
    current: View | null;
    disabled: boolean;
    adding?: boolean;
    onNavigate: (view: View) => void;
    onAdd: () => void;
    profileMenu: Snippet;
    children: Snippet;
  } = $props();

  const views: { id: View; label: string }[] = [
    { id: "library", label: "Library" },
    { id: "practice", label: "Practise" },
    { id: "matching", label: "Match pairs" },
  ];
</script>

{#snippet icon(view: View)}
  <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
    {#if view === "library"}
      <rect x="3.5" y="8.5" width="17" height="12" rx="2" />
      <path d="M6 5.5h12M8.5 2.75h7" />
    {:else if view === "practice"}
      <path d="M19.5 10.5A7.75 7.75 0 0 0 5.4 7.2M4.5 3.5v4h4" />
      <path d="M4.5 13.5a7.75 7.75 0 0 0 14.1 3.3M19.5 20.5v-4h-4" />
    {:else}
      <rect x="3.5" y="3.5" width="7" height="7" rx="1.75" />
      <rect x="13.5" y="3.5" width="7" height="7" rx="1.75" />
      <rect x="3.5" y="13.5" width="7" height="7" rx="1.75" />
      <rect x="13.5" y="13.5" width="7" height="7" rx="1.75" />
    {/if}
  </svg>
{/snippet}

<div class="app-shell">
  <header class="app-header">
    <a
      class="brand"
      href={`/p/${profileId}/library`}
      onclick={(event) => {
        event.preventDefault();
        onNavigate("library");
      }}
      aria-label="Keepsake library"
    >
      <img src="/keepsake-logo.png" alt="Keepsake" width="2172" height="724" />
    </a>
    {#if back}
      <button
        class="back"
        aria-label={`Back to ${back.label}`}
        {disabled}
        onclick={back.onBack}
      >
        <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
          <path d="M15 5.5 8.5 12l6.5 6.5" />
        </svg>
        <span>{back.label}</span>
      </button>
    {/if}
    <div class="title">
      <h1>{title}</h1>
      {#if detail !== null}<span class="detail">{detail}</span>{/if}
    </div>
    <nav aria-label="Main navigation">
      {#each views as view (view.id)}
        <button
          class:active={current === view.id}
          aria-current={current === view.id ? "page" : undefined}
          {disabled}
          onclick={() => onNavigate(view.id)}
        >
          {@render icon(view.id)}
          <span>{view.label}</span>
        </button>
      {/each}
    </nav>
    <div class="actions">
      <button class="add" {disabled} onclick={onAdd}>
        <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
          <path d={adding ? "M6.5 6.5l11 11M17.5 6.5l-11 11" : "M12 4.5v15M4.5 12h15"} />
        </svg>
        <span class="add-label">{adding ? "Close form" : "Add a fact"}</span>
      </button>
      {@render profileMenu()}
    </div>
  </header>

  <main>
    {@render children()}
  </main>
</div>

<style>
  .app-shell {
    max-width: 1240px;
    margin: auto;
    padding: 0 3rem;
  }

  .app-header {
    position: sticky;
    top: 0;
    z-index: 5;
    min-height: 64px;
    display: flex;
    align-items: center;
    gap: 1.5rem;
    background: #f5f8f7ee;
    border-bottom: 1px solid #cedbd5;
  }

  .brand {
    flex-shrink: 0;
    width: 120px;
  }

  .brand img {
    width: 100%;
    height: auto;
    display: block;
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    min-width: 0;
  }

  h1 {
    font-family:
      "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Palatino, serif;
    font-size: 1.5rem;
    font-weight: 500;
    letter-spacing: -0.03em;
    line-height: 1.2;
    margin: 0;
    white-space: nowrap;
  }

  .detail {
    color: #64786c;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  nav {
    display: flex;
    gap: 0.35rem;
    margin-left: auto;
  }

  nav button {
    background: none;
    color: #51675e;
    padding: 0.6rem 0.9rem;
  }

  nav button.active {
    color: #18362f;
    background: #e3ede7;
    font-weight: 600;
  }

  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  nav svg {
    display: none;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .add {
    background: #244c44;
    color: #fff;
    padding: 0.7rem 1.1rem;
    font-weight: 600;
  }

  .add:hover {
    background: #183c34;
  }

  .add svg {
    display: none;
  }

  /* Wider screens keep the navigation in view, so only phones show Back. */
  .back {
    display: none;
  }

  /* The Home Screen app's edge swipe: horizontal drags reach the page, and the
     page slides over a darker backdrop while it follows the finger. */
  :global(html.edge-swipe) .app-shell {
    touch-action: pan-y pinch-zoom;
  }

  :global(html.swiping-back body) {
    background: #dde5e1;
  }

  :global(html.swiping-back) main {
    background: #f5f8f7;
    box-shadow: -8px 0 24px #18362f29;
  }

  main {
    padding: 1.25rem 0 4rem;
  }

  @media (max-width: 900px) {
    .app-shell {
      padding: 0 1.5rem;
    }

    .app-header {
      gap: 1rem;
    }

    .brand {
      display: none;
    }

    nav button {
      padding: 0.6rem 0.7rem;
      font-size: 0.9rem;
    }

    .add {
      font-size: 0.9rem;
      padding: 0.65rem 0.8rem;
    }
  }

  /* Phones: a navigation bar with the title centred, and a tab bar at the bottom. */
  @media (max-width: 700px) {
    .app-shell {
      padding: 0;
    }

    .app-header {
      display: grid;
      grid-template-columns: 1fr auto 1fr;
      gap: 0.5rem;
      min-height: 0;
      height: calc(48px + env(safe-area-inset-top));
      /* The bar covers the safe area itself once the page scrolls under it. */
      margin-top: calc(-1 * env(safe-area-inset-top));
      padding: env(safe-area-inset-top) 0.75rem 0;
      background: none;
      border-bottom: 0.5px solid #18362f33;
    }

    /* A backdrop filter on the header itself would make it the containing block
       of the fixed tab bar, so the blur sits on a layer behind it instead. */
    .app-header::before {
      content: "";
      position: absolute;
      inset: 0;
      z-index: -1;
      background: #f5f8f7e6;
      -webkit-backdrop-filter: saturate(1.8) blur(20px);
      backdrop-filter: saturate(1.8) blur(20px);
    }

    .back {
      grid-column: 1;
      justify-self: start;
      display: flex;
      align-items: center;
      max-width: 100%;
      min-width: 0;
      padding: 0.4rem 0.25rem 0.4rem 0;
      margin-left: -0.35rem;
      background: none;
      color: #244c44;
      font-size: 1rem;
    }

    .back svg {
      flex-shrink: 0;
    }

    .back span {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .title {
      grid-column: 2;
      flex-direction: column;
      align-items: center;
      gap: 0;
    }

    h1 {
      font-family: inherit;
      font-size: 1.05rem;
      font-weight: 600;
      letter-spacing: 0;
      line-height: 1.25;
    }

    .detail {
      height: 0.9rem;
      font-size: 0.72rem;
      line-height: 0.9rem;
    }

    .actions {
      grid-column: 3;
      justify-self: end;
      gap: 0.25rem;
    }

    .add {
      display: grid;
      place-items: center;
      width: 40px;
      height: 40px;
      padding: 0;
      background: none;
      color: #244c44;
    }

    .add:hover {
      background: none;
    }

    .add svg {
      display: block;
    }

    .add-label {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
    }

    nav {
      position: fixed;
      z-index: 5;
      left: 0;
      right: 0;
      bottom: 0;
      gap: 0;
      margin: 0;
      padding: 0 env(safe-area-inset-right) env(safe-area-inset-bottom)
        env(safe-area-inset-left);
      background: #f5f8f7f0;
      -webkit-backdrop-filter: saturate(1.8) blur(20px);
      backdrop-filter: saturate(1.8) blur(20px);
      border-top: 0.5px solid #18362f33;
    }

    nav button {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 0.15rem;
      height: 52px;
      padding: 0.4rem 0 0.25rem;
      border-radius: 0;
      font-size: 0.68rem;
      font-weight: 500;
      color: #6b7d74;
    }

    nav button.active {
      background: none;
      color: #244c44;
      font-weight: 600;
    }

    nav svg {
      display: block;
      width: 26px;
      height: 26px;
    }

    nav button.active svg {
      fill: #244c4426;
    }

    main {
      padding: 0.75rem 1rem calc(52px + 2rem);
    }
  }
</style>
