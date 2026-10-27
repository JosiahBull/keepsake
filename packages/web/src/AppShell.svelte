<script lang="ts">
  import type { Snippet } from "svelte";

  type View = "library" | "practice" | "matching";

  // Every screen shares this header, so switching screens never moves it.
  let {
    profileId,
    current,
    disabled,
    adding = false,
    onNavigate,
    onAdd,
    profileMenu,
    children,
  }: {
    profileId: number;
    current: View | null;
    disabled: boolean;
    adding?: boolean;
    onNavigate: (view: View) => void;
    onAdd: () => void;
    profileMenu: Snippet;
    children: Snippet;
  } = $props();
</script>

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
    <nav aria-label="Main navigation">
      <button
        class:active={current === "library"}
        aria-current={current === "library" ? "page" : undefined}
        {disabled}
        onclick={() => onNavigate("library")}
      >
        Library
      </button>
      <button
        class:active={current === "practice"}
        aria-current={current === "practice" ? "page" : undefined}
        {disabled}
        onclick={() => onNavigate("practice")}
      >
        Practise
      </button>
      <button
        class:active={current === "matching"}
        aria-current={current === "matching" ? "page" : undefined}
        {disabled}
        onclick={() => onNavigate("matching")}
      >
        Match pairs
      </button>
    </nav>
    <button class="primary add-top" {disabled} onclick={onAdd}>
      {adding ? "Close form" : "Add a fact"}
    </button>
    {@render profileMenu()}
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
    min-height: 112px;
    display: flex;
    align-items: center;
    gap: 2.5rem;
    border-bottom: 1px solid #cedbd5;
  }

  .brand {
    flex-shrink: 0;
    width: 165px;
  }

  .brand img {
    width: 100%;
    height: auto;
    display: block;
  }

  nav {
    display: flex;
    gap: 0.35rem;
    margin-left: auto;
  }

  nav button {
    background: none;
    color: #51675e;
    padding: 0.7rem 0.9rem;
  }

  nav button.active {
    color: #18362f;
    background: #e3ede7;
    font-weight: 600;
  }

  .primary {
    background: #244c44;
    color: #fff;
    padding: 0.85rem 1.2rem;
    font-weight: 600;
  }

  .primary:hover {
    background: #183c34;
  }

  main {
    padding: 2rem 0 4rem;
  }

  @media (max-width: 850px) {
    .app-shell {
      padding: 0 1.5rem;
    }

    .app-header {
      gap: 1rem;
    }

    .brand {
      width: 140px;
    }

    nav button {
      padding: 0.65rem;
      font-size: 0.9rem;
    }

    .add-top {
      font-size: 0.85rem;
      padding: 0.75rem 0.8rem;
    }
  }

  @media (max-width: 580px) {
    .app-shell {
      padding: 0 1rem;
    }

    .app-header {
      flex-wrap: wrap;
      gap: 0.5rem;
      padding: 1.1rem 0 0.8rem;
      min-height: auto;
    }

    .brand {
      width: 145px;
    }

    nav {
      order: 3;
      width: 100%;
      margin: 0.8rem 0 0;
    }

    nav button {
      flex: 1;
    }

    .add-top {
      display: none;
    }

    main {
      padding-top: 1.5rem;
    }
  }
</style>
