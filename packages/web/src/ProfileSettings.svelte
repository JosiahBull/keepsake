<script lang="ts">
  import { onMount, untrack } from "svelte";
  import type { components } from "@keepsake/client";
  import { json, errorMessage, type Profile } from "./profiles";
  import AgentAccess from "./AgentAccess.svelte";
  import Reminders from "./Reminders.svelte";
  import { play, setSoundsOn, soundsOn } from "./sounds";

  let {
    profile,
    blocked = $bindable(false),
    dirty = $bindable(false),
    onSaved,
    onCancel,
  }: {
    profile: Profile;
    blocked?: boolean;
    dirty?: boolean;
    onSaved: (profile: Profile) => void;
    onCancel: () => void;
  } = $props();
  const id = untrack(() => profile.id);
  let originalName = $state(untrack(() => profile.name));
  let name = $state(untrack(() => profile.name));
  let settings = $state<components["schemas"]["SchedulerSettings"] | null>(
    null,
  );
  let newLimit = $state(5);
  let budget = $state(15);
  let timezone = $state("Europe/London");
  let loading = $state(true);
  let saving = $state(false);
  let unresolved = $state(false);
  let error = $state("");
  let notice = $state("");
  let sounds = $state(soundsOn());
  const controller = new AbortController();
  $effect(() => {
    blocked = saving || unresolved;
    dirty = Boolean(
      settings &&
      (name !== originalName ||
        newLimit !== settings.new_limit ||
        budget * 60 !== settings.daily_budget_seconds ||
        timezone !== settings.timezone),
    );
  });
  onMount(() => {
    void load();
    return () => controller.abort();
  });
  async function load() {
    loading = true;
    error = "";
    try {
      const [savedSettings, savedProfile] = await Promise.all([
        json<components["schemas"]["SchedulerSettings"]>(
          `/api/profiles/${id}/study/settings`,
          { signal: controller.signal },
        ),
        json<Profile>(`/api/profiles/${id}`, { signal: controller.signal }),
      ]);
      if (controller.signal.aborted) return;
      settings = savedSettings;
      originalName = savedProfile.name;
      name = savedProfile.name;
      onSaved(savedProfile);
      unresolved = false;
      newLimit = settings!.new_limit;
      budget = settings!.daily_budget_seconds / 60;
      timezone = settings!.timezone;
    } catch (caught) {
      if (!controller.signal.aborted)
        error = `Could not load settings. ${errorMessage(caught)}`;
    } finally {
      loading = false;
    }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!settings || saving) return;
    saving = true;
    unresolved = true;
    error = "";
    notice = "";
    try {
      // Each operation is retry-safe. Keep the returned settings version after a partial save.
      settings = await json(`/api/profiles/${id}/study/settings`, {
        method: "PUT",
        body: JSON.stringify({
          expected_version: settings.version,
          new_limit: newLimit,
          daily_budget_seconds: Math.round(budget * 60),
          timezone: timezone.trim(),
        }),
      });
      const updated = await json<Profile>(`/api/profiles/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ name: name.trim() }),
      });
      originalName = updated.name;
      name = updated.name;
      onSaved(updated);
      unresolved = false;
      notice = "Changes saved.";
    } catch (caught) {
      error = `Could not save all changes. ${errorMessage(caught)} Retry saving, or discard changes and reload saved settings.`;
    } finally {
      saving = false;
    }
  }
</script>

<section class="settings">
  <h1>Profile settings</h1>
  {#if error}<p class="message error" role="alert">{error}</p>
    {#if settings}<button
        class="quiet recover"
        disabled={saving || loading}
        onclick={load}>Discard changes and reload</button
      >{/if}{/if}
  {#if loading}<p role="status">Loading settings…</p>
  {:else if settings}
    <form onsubmit={save}>
      <label
        >Name<input
          bind:value={name}
          required
          maxlength="80"
          disabled={saving}
        /></label
      >
      <p class="muted">CLI handle: <code>{profile.handle}</code></p>
      <div class="settings-grid">
        <label
          >New facts per day<input
            type="number"
            bind:value={newLimit}
            min="0"
            max="100"
            required
            disabled={saving}
          /></label
        >
        <label
          >Daily study budget (minutes)<input
            type="number"
            bind:value={budget}
            min="1"
            max="240"
            step="any"
            required
            disabled={saving}
          /></label
        >
      </div>
      <label
        >Timezone<input
          bind:value={timezone}
          required
          maxlength="100"
          list="timezones"
          disabled={saving}
        /><datalist id="timezones"
          ><option value="Europe/London"></option><option
            value="Pacific/Auckland"
          ></option></datalist
        ></label
      >
      <p class="muted">
        These settings apply to {profile.name}. Household profiles have no
        passwords.
      </p>
      <div class="actions">
        <button class="primary" disabled={saving}
          >{saving ? "Saving…" : "Save changes"}</button
        ><button
          type="button"
          class="quiet"
          disabled={saving || unresolved}
          onclick={onCancel}>Cancel</button
        >
      </div>
      {#if notice}<p role="status">{notice}</p>{/if}
    </form>
    <!-- Reminder windows follow the saved study timezone. -->
    {#key settings.timezone}<Reminders {profile} />{/key}
    <section class="sounds" aria-labelledby="sounds-heading">
      <h2 id="sounds-heading">Sounds</h2>
      <label class="toggle"
        ><input
          type="checkbox"
          checked={sounds}
          onchange={(event) => {
            sounds = event.currentTarget.checked;
            setSoundsOn(sounds);
            if (sounds) play("file");
          }}
        />Play sounds on this device</label
      >
    </section>
    <AgentAccess {profile} />
  {:else}<button class="quiet" onclick={load}>Try again</button>{/if}
</section>

<style>
  .recover {
    margin-bottom: 1.5rem;
  }
  .settings {
    max-width: 580px;
  }
  h1 {
    font:
      500 2rem/1.2 "Iowan Old Style",
      Palatino,
      serif;
    letter-spacing: -0.035em;
  }
  form {
    display: grid;
    gap: 1.1rem;
  }
  label {
    display: block;
    font-size: 0.9rem;
    font-weight: 600;
  }
  label input {
    margin-top: 0.5rem;
    font-weight: 400;
  }
  .settings-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
  .muted {
    color: #64786c;
    font-size: 0.85rem;
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 0.8rem;
  }
  .primary {
    padding: 0.85rem 1.2rem;
    background: #244c44;
    color: white;
    font-weight: 600;
  }
  .quiet {
    padding: 0.8rem 1rem;
    background: transparent;
    color: #244c44;
    border: 1px solid #c0d0c7;
  }
  .message {
    padding: 1rem 1.2rem;
    border-radius: 6px;
  }
  .error {
    background: #f9e9e3;
    color: #833d2b;
  }
  .sounds {
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
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-weight: 400;
  }
  .toggle input {
    width: auto;
    margin: 0;
    accent-color: #244c44;
  }
  @media (max-width: 580px) {
    .settings-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
