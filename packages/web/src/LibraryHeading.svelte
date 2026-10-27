<script lang="ts">
  import type { Snippet } from "svelte";

  type Tab = "library" | "sharing" | "sent";

  // My library, Shared with me and Sent facts share one heading and tab bar.
  let {
    current,
    disabled,
    addDisabled,
    adding = false,
    onAdd,
    onNavigate,
    children,
  }: {
    current: Tab;
    disabled: boolean;
    addDisabled: boolean;
    adding?: boolean;
    onAdd: () => void;
    onNavigate: (tab: Tab) => void;
    children?: Snippet;
  } = $props();

  const tabs: { id: Tab; label: string }[] = [
    { id: "library", label: "My library" },
    { id: "sharing", label: "Shared with me" },
    { id: "sent", label: "Sent facts" },
  ];
</script>

<div class="page-heading">
  <h1>Library</h1>
  {@render children?.()}
  <button class="primary mobile-add" disabled={addDisabled} onclick={onAdd}
    >{adding ? "Close form" : "Add a fact"}</button
  >
</div>

<div class="library-tabs">
  {#each tabs as tab}<button
      class:active={current === tab.id}
      aria-current={current === tab.id ? "page" : undefined}
      disabled={disabled || current === tab.id}
      onclick={() => onNavigate(tab.id)}>{tab.label}</button
    >{/each}
</div>

<style>
  .page-heading {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  h1 {
    font-family:
      "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Palatino, serif;
    font-size: 2rem;
    font-weight: 500;
    letter-spacing: -0.035em;
    line-height: 1.2;
    margin-bottom: 0;
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

  .mobile-add {
    display: none;
  }

  .library-tabs {
    display: flex;
    gap: 0.4rem;
    border-bottom: 1px solid #cedbd5;
    margin-bottom: 1.4rem;
  }

  .library-tabs button {
    padding: 0.7rem 1rem;
    color: #244c44;
    background: transparent;
  }

  .library-tabs .active {
    background: #e3ede7;
    opacity: 1;
    font-weight: 600;
  }

  @media (max-width: 580px) {
    .mobile-add {
      display: block;
      margin-left: auto;
      font-size: 0.85rem;
      padding: 0.75rem 0.8rem;
    }

    .library-tabs button {
      padding: 0.7rem 0.55rem;
      font-size: 0.9rem;
    }
  }
</style>
