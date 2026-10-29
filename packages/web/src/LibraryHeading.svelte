<script lang="ts">
  import type { Snippet } from "svelte";

  type Tab = "library" | "sharing" | "sent";

  // My library, Shared with me and Sent facts share one segmented control.
  // Search, when there is any, sits beside it, or below it on phones.
  let {
    current,
    disabled,
    onNavigate,
    children,
  }: {
    current: Tab;
    disabled: boolean;
    onNavigate: (tab: Tab) => void;
    children?: Snippet;
  } = $props();

  const tabs: { id: Tab; label: string }[] = [
    { id: "library", label: "My library" },
    { id: "sharing", label: "Shared with me" },
    { id: "sent", label: "Sent facts" },
  ];
</script>

<div class="library-heading">
  <div class="library-tabs">
    {#each tabs as tab}<button
        class:active={current === tab.id}
        aria-current={current === tab.id ? "page" : undefined}
        disabled={disabled || current === tab.id}
        onclick={() => onNavigate(tab.id)}>{tab.label}</button
      >{/each}
  </div>
  {@render children?.()}
</div>

<style>
  .library-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem 1rem;
    margin-bottom: 0.75rem;
  }

  .library-tabs {
    display: flex;
    gap: 2px;
    padding: 2px;
    background: #e3ebe7;
    border-radius: 9px;
  }

  .library-tabs button {
    padding: 0.4rem 1rem;
    color: #244c44;
    background: transparent;
    border-radius: 7px;
    font-size: 0.9rem;
    font-weight: 500;
    line-height: 1.4;
  }

  .library-tabs .active {
    background: #fff;
    box-shadow:
      0 1px 3px #18362f1f,
      0 0 0 0.5px #18362f0f;
    opacity: 1;
    font-weight: 600;
  }

  @media (max-width: 700px) {
    .library-tabs {
      flex: 1 1 100%;
    }

    .library-tabs button {
      flex: 1;
      padding: 0.35rem 0.25rem;
      font-size: 0.82rem;
      white-space: nowrap;
    }
  }
</style>
