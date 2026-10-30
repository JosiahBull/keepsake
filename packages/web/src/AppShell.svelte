<script lang="ts">
  import type { Snippet } from "svelte";

  type View = "library" | "practice" | "matching";

  // Every screen shares this navigation bar and tab bar, so switching screens never moves them.
  let {
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

  <!-- After the page, so keyboard order follows the screen from top to bottom. -->
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
</div>

<style>
  /* An app layout at every width: a navigation bar at the top with Back, the
     title and actions, and a tab bar fixed at the bottom. */
  .app-header {
    position: sticky;
    top: 0;
    z-index: 5;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 1rem;
    height: 56px;
    padding: 0 1.5rem;
    background: #f5f8f7e6;
    -webkit-backdrop-filter: saturate(1.8) blur(20px);
    backdrop-filter: saturate(1.8) blur(20px);
    border-bottom: 0.5px solid #18362f33;
  }

  .back {
    grid-column: 1;
    justify-self: start;
    display: flex;
    align-items: center;
    max-width: 100%;
    min-width: 0;
    padding: 0.4rem 0.6rem 0.4rem 0.1rem;
    margin-left: -0.4rem;
    background: none;
    color: #244c44;
    font-size: 1rem;
  }

  .back:hover {
    background: #e3ede7;
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
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    line-height: 1.25;
    white-space: nowrap;
  }

  .detail {
    height: 0.95rem;
    color: #64786c;
    font-size: 0.75rem;
    line-height: 0.95rem;
    white-space: nowrap;
  }

  .actions {
    grid-column: 3;
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .add {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.45rem 0.75rem 0.45rem 0.5rem;
    background: none;
    color: #244c44;
    font-weight: 600;
  }

  .add:hover {
    background: #e3ede7;
  }

  main {
    max-width: 1240px;
    margin: auto;
    padding: 1.25rem 3rem calc(56px + 2.5rem);
  }

  nav {
    position: fixed;
    z-index: 5;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    justify-content: center;
    padding: 0 env(safe-area-inset-right) env(safe-area-inset-bottom)
      env(safe-area-inset-left);
    background: #f5f8f7f0;
    -webkit-backdrop-filter: saturate(1.8) blur(20px);
    backdrop-filter: saturate(1.8) blur(20px);
    border-top: 0.5px solid #18362f33;
  }

  /* Wide tab bars set each icon beside its label, as iPad apps do. */
  nav button {
    flex: 0 1 11rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    height: 56px;
    border-radius: 0;
    background: none;
    color: #6b7d74;
    font-size: 0.9rem;
    font-weight: 500;
  }

  nav button.active {
    color: #244c44;
    font-weight: 600;
  }

  nav svg {
    width: 26px;
    height: 26px;
  }

  nav button.active svg {
    fill: #244c4426;
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
    /* The moving page holds fixed descendants, such as the loading mark, so it
       must stay screen height for them to stay centred. */
    min-height: 100dvh;
    background: #f5f8f7;
    box-shadow: -8px 0 24px #18362f29;
  }

  @media (max-width: 900px) {
    main {
      padding-left: 1.5rem;
      padding-right: 1.5rem;
    }
  }

  /* Phones: a compact bar, icon-only actions, and tabs stacking icon over label. */
  @media (max-width: 700px) {
    .app-header {
      gap: 0.5rem;
      height: calc(48px + env(safe-area-inset-top));
      /* The bar covers the safe area itself once the page scrolls under it. */
      margin-top: calc(-1 * env(safe-area-inset-top));
      padding: env(safe-area-inset-top) 0.75rem 0;
    }

    .back {
      padding-right: 0.25rem;
      margin-left: -0.35rem;
    }

    .back:hover {
      background: none;
    }

    h1 {
      font-size: 1.05rem;
    }

    .detail {
      height: 0.9rem;
      font-size: 0.72rem;
      line-height: 0.9rem;
    }

    .actions {
      gap: 0.25rem;
    }

    .add {
      display: grid;
      place-items: center;
      width: 40px;
      height: 40px;
      padding: 0;
    }

    .add:hover {
      background: none;
    }

    .add-label {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
    }

    main {
      padding: 0.75rem 1rem calc(52px + 2rem);
    }

    nav button {
      flex: 1;
      flex-direction: column;
      gap: 0.15rem;
      height: 52px;
      padding: 0.4rem 0 0.25rem;
      font-size: 0.68rem;
    }
  }
</style>
