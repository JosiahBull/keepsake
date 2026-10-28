<script lang="ts">
  import { onMount, untrack } from "svelte";
  import type { components } from "@keepsake/client";
  import { json, errorMessage, type Profile } from "./profiles";

  type Access = components["schemas"]["AgentAccess"];
  type Mode = components["schemas"]["McpMode"];

  let { profile }: { profile: Profile } = $props();
  const id = untrack(() => profile.id);
  const endpoint = `${location.origin}/mcp/${untrack(() => profile.handle)}`;
  const ORDER: Mode[] = ["off", "read", "write"];
  const LABELS: Record<Mode, string> = {
    off: "Off",
    read: "Read only",
    write: "Read and add facts",
  };
  let access = $state<Access | null>(null);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  // The server refuses a choice above its ceiling, so only offer what it allows.
  const allowed = $derived(
    access ? ORDER.slice(0, ORDER.indexOf(access.ceiling) + 1) : [],
  );

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      access = await json<Access>(`/api/profiles/${id}/agent-access`);
    } catch (caught) {
      error = `Could not load agent access. ${errorMessage(caught)}`;
    }
  }

  async function choose(mode: Mode) {
    if (busy) return;
    busy = true;
    error = "";
    notice = "";
    try {
      access = await json<Access>(`/api/profiles/${id}/agent-access`, {
        method: "PUT",
        body: JSON.stringify({ mode }),
      });
      notice = `Agent access: ${LABELS[access.effective].toLowerCase()}.`;
    } catch (caught) {
      error = `Could not change agent access. ${errorMessage(caught)}`;
    } finally {
      busy = false;
    }
  }
</script>

<section class="agent-access" aria-labelledby="agent-access-heading">
  <h2 id="agent-access-heading">Agent access</h2>
  <p>
    Let an AI agent such as Claude Code search {profile.name}'s library and,
    if you allow it, add facts. Everything the agent reads is sent to its
    model provider.
  </p>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}
  {#if access}
    {#if access.ceiling === "off"}
      <p class="muted">
        Not available on this server. Its operator can allow it by setting
        <code>KEEPSAKE_MCP</code> to <code>read</code> or <code>write</code>
        and restarting.
      </p>
    {:else}
      <label
        >Access<select
          value={access.effective}
          disabled={busy}
          onchange={(event) => choose(event.currentTarget.value as Mode)}
        >
          {#each allowed as mode}<option value={mode}>{LABELS[mode]}</option
            >{/each}
        </select></label
      >
      {#if access.ceiling === "read"}
        <p class="muted">This server allows read-only access at most.</p>
      {/if}
      {#if access.effective !== "off"}
        <p>Connect an agent to <code>{endpoint}</code>. With Claude Code:</p>
        <pre><code
            >claude mcp add --transport http keepsake-{profile.handle} {endpoint}</code
          ></pre>
        <p class="muted">Changes apply to the agent's next request.</p>
      {/if}
    {/if}
  {/if}
  {#if notice}<p role="status">{notice}</p>{/if}
</section>

<style>
  .agent-access {
    display: grid;
    gap: 1rem;
    margin-top: 2.5rem;
    padding-top: 2rem;
    border-top: 1px solid #cedbd5;
  }
  h2 {
    font:
      500 1.5rem/1.3 "Iowan Old Style",
      Palatino,
      serif;
    margin: 0;
  }
  p {
    margin: 0;
  }
  label {
    display: block;
    font-size: 0.9rem;
    font-weight: 600;
  }
  label select {
    display: block;
    margin-top: 0.5rem;
    font-weight: 400;
  }
  pre {
    margin: 0;
    padding: 0.8rem 1rem;
    /* Wrapped so the whole command shows; it still copies as one line. */
    white-space: pre-wrap;
    background: #fff;
    border: 1px solid #cedbd5;
    border-radius: 6px;
    font-size: 0.85rem;
  }
  code {
    overflow-wrap: anywhere;
  }
  .muted {
    color: #64786c;
    font-size: 0.85rem;
  }
  .message {
    padding: 1rem 1.2rem;
    border-radius: 6px;
  }
  .error {
    background: #f9e9e3;
    color: #833d2b;
  }
</style>
