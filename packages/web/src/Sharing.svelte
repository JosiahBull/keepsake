<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    json,
    errorMessage,
    type Profile,
    type SharePage,
    type Screen,
  } from "./profiles";
  let {
    profile,
    direction,
    onNavigate,
    blocked = $bindable(false),
  }: {
    profile: Profile;
    direction: "inbox" | "sent";
    onNavigate: (screen: Screen) => void;
    blocked?: boolean;
  } = $props();
  const id = untrack(() => profile.id);
  let page = $state<SharePage | null>(null);
  let selected = $state<number[]>([]);
  const batchSelection = $derived(
    (page?.items.filter((share) => share.status === "pending").length ?? 0) > 1,
  );
  let loading = $state(true);
  let saving = $state(false);
  let error = $state("");
  let notice = $state("");
  let pending = $state<{ path: string; body: unknown } | null>(null);
  let requestNumber = 0;
  const controller = new AbortController();
  $effect(() => {
    blocked = saving || pending !== null;
  });
  onMount(() => {
    void load();
    return () => {
      requestNumber += 1;
      controller.abort();
    };
  });
  async function load(before?: number) {
    const request = ++requestNumber;
    loading = true;
    error = "";
    try {
      const result = await json<SharePage>(
        `/api/profiles/${id}/shares?direction=${direction}&limit=25${before ? `&before_id=${before}` : ""}`,
        { signal: controller.signal },
      );
      if (request === requestNumber) {
        page =
          before && page
            ? { ...result, items: [...page.items, ...result.items] }
            : result;
        selected = selected.filter((shareId) =>
          page!.items.some(
            (share) => share.id === shareId && share.status === "pending",
          ),
        );
      }
    } catch (caught) {
      if (request === requestNumber && !controller.signal.aborted)
        error = `Could not load shared facts. ${errorMessage(caught)}`;
    } finally {
      if (request === requestNumber) loading = false;
    }
  }
  async function act(path: string, body: unknown = {}) {
    if (saving) return;
    pending ??= { path, body };
    saving = true;
    error = "";
    notice = "";
    try {
      await json(`/api/profiles/${id}/shares/${pending.path}`, {
        method: "POST",
        body: JSON.stringify(pending.body),
      });
      notice =
        pending.path === "accept-batch" || pending.path.endsWith("/accept")
          ? "Copies added to your library."
          : pending.path.endsWith("/decline")
            ? "Share declined."
            : "Share withdrawn.";
      pending = null;
      selected = [];
      await load();
    } catch (caught) {
      error = `Could not save this change. ${errorMessage(caught)}`;
    } finally {
      saving = false;
    }
  }
</script>

<h1>Library</h1>
<div class="tabs">
  <button disabled={blocked} onclick={() => onNavigate("library")}
    >My library</button
  ><button
    class:active={direction === "inbox"}
    disabled={blocked}
    onclick={() => onNavigate("sharing")}>Shared with me</button
  ><button
    class:active={direction === "sent"}
    disabled={blocked}
    onclick={() => onNavigate("sent")}>Sent facts</button
  >
</div>
{#if notice}<p class="message" role="status">{notice}</p>{/if}
{#if error}<div class="message error" role="alert">
    {error}<button
      class="text-button"
      disabled={saving}
      onclick={() => (pending ? act(pending.path, pending.body) : load())}
      >Retry {pending ? "save" : "load"}</button
    >
  </div>{/if}
{#if direction === "inbox" && page?.items.some((share) => share.status === "pending")}
  <p class="muted">
    Accept copies with fresh review schedules. Later edits stay separate.
  </p>
  {#if batchSelection}
    <div class="selection">
      <button
        class="primary"
        disabled={blocked || selected.length === 0}
        onclick={() => act("accept-batch", { share_ids: selected })}
        >Add {selected.length || "selected"}
        {selected.length === 1 ? "fact" : "facts"} to my library</button
      ><button
        class="quiet"
        disabled={blocked}
        onclick={() =>
          (selected = page!.items
            .filter((item) => item.status === "pending")
            .slice(0, 20)
            .map((item) => item.id))}>Select pending facts</button
      >
    </div>
  {/if}
{/if}
{#if loading && !page}<p role="status">Loading shared facts…</p>{/if}
{#each page?.items ?? [] as share (share.id)}
  <article class="share-row">
    {#if direction === "inbox" && share.status === "pending" && batchSelection}<input
        class="share-select"
        type="checkbox"
        aria-label={`Select ${share.question}`}
        checked={selected.includes(share.id)}
        disabled={blocked ||
          (!selected.includes(share.id) && selected.length >= 20)}
        onchange={() =>
          (selected = selected.includes(share.id)
            ? selected.filter((value) => value !== share.id)
            : [...selected, share.id])}
      />{/if}
    <div class="share-copy">
      <h2>{share.question}</h2>
      <p class="muted">
        {direction === "inbox"
          ? `From ${share.sender_name}`
          : `To ${share.recipient_name}`} · {share.status === "pending"
          ? "Waiting for acceptance"
          : share.status === "accepted"
            ? "Accepted"
            : share.status === "declined"
              ? "Declined"
              : "Withdrawn"}
      </p>
    </div>
    {#if share.status === "pending"}<div class="actions">
        {#if direction === "inbox"}<button
            class="quiet"
            disabled={blocked}
            onclick={() => act(`${share.id}/accept`)}>Add to my library</button
          ><button
            class="text-button"
            disabled={blocked}
            onclick={() => act(`${share.id}/decline`)}>Decline</button
          >{:else}<button
            class="quiet"
            disabled={blocked}
            onclick={() => act(`${share.id}/withdraw`)}>Withdraw</button
          >{/if}
      </div>{/if}
  </article>
{:else}
  {#if !loading && !error}<p class="empty">
      {direction === "inbox"
        ? "No shared facts yet."
        : "You haven’t shared any facts yet."}
    </p>{/if}
{/each}
{#if page?.next_cursor}<button
    class="quiet more"
    disabled={loading || blocked}
    onclick={() => load(page!.next_cursor ?? undefined)}
    >{loading ? "Loading…" : "Show more"}</button
  >{/if}

<style>
  h1 {
    font:
      500 2rem/1.2 "Iowan Old Style",
      Palatino,
      serif;
    letter-spacing: -0.035em;
  }
  .tabs {
    display: flex;
    gap: 0.4rem;
    border-bottom: 1px solid #cedbd5;
    margin-bottom: 1.4rem;
  }
  .tabs button {
    padding: 0.7rem 1rem;
    color: #244c44;
    background: transparent;
  }
  .tabs .active {
    background: #e3ede7;
    font-weight: 600;
  }
  .selection {
    display: flex;
    gap: 0.8rem;
    flex-wrap: wrap;
    margin: 1.5rem 0;
  }
  .share-row {
    padding: 1.2rem 0;
    border-bottom: 1px solid #cedbd5;
    display: flex;
    align-items: flex-start;
    gap: 1rem;
  }
  .share-copy {
    flex: 1;
  }
  .share-row h2 {
    font-size: 1.05rem;
    line-height: 1.6;
    margin: 0 0 0.5rem;
    overflow-wrap: anywhere;
  }
  .share-select {
    width: auto;
    margin-top: 0.4rem;
    accent-color: #244c44;
  }
  .muted {
    font-size: 0.85rem;
    color: #64786c;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .primary {
    padding: 0.85rem 1.2rem;
    background: #244c44;
    color: white;
    font-weight: 600;
  }
  .quiet {
    padding: 0.7rem 1rem;
    background: transparent;
    color: #244c44;
    border: 1px solid #c0d0c7;
  }
  .text-button {
    background: none;
    color: #244c44;
    text-decoration: underline;
    padding: 0.5rem;
  }
  .message {
    background: #e3ede7;
    padding: 1rem 1.2rem;
    border-radius: 6px;
  }
  .error {
    color: #833d2b;
    background: #f9e9e3;
  }
  .empty {
    padding: 2rem 0;
  }
  .more {
    margin-top: 1.5rem;
  }
  @media (max-width: 580px) {
    .share-row {
      flex-wrap: wrap;
    }
    .share-copy {
      flex-basis: calc(100% - 3rem);
    }
    .share-row:has(.share-select) .actions {
      margin-left: 2rem;
    }
    .tabs button {
      padding: 0.7rem 0.55rem;
      font-size: 0.9rem;
    }
  }
</style>
