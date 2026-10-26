<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { json, errorMessage, type Profile } from "./profiles";
  import {
    disableReminders,
    enableReminders,
    reminderSupport,
    remindersOn,
    testReminder,
    type ReminderSettings,
  } from "./pwa";

  let { profile }: { profile: Profile } = $props();
  const id = untrack(() => profile.id);
  let settings = $state<ReminderSettings | null>(null);
  let support = $state(reminderSupport());
  let on = $state(false);
  let time = $state("08:00");
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  const savedTime = $derived(settings ? clock(settings.minute_of_day) : "");
  const otherDevices = $derived(
    settings ? settings.devices - (on ? 1 : 0) : 0,
  );

  function clock(minutes: number): string {
    return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
  }

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      const [saved, enabled] = await Promise.all([
        json<ReminderSettings>(`/api/profiles/${id}/reminders`),
        remindersOn(id),
      ]);
      settings = saved;
      on = enabled;
      time = clock(saved.minute_of_day);
    } catch (caught) {
      error = `Could not load reminders. ${errorMessage(caught)}`;
    }
  }

  async function act(action: () => Promise<void>, failure: string) {
    if (busy) return;
    busy = true;
    error = "";
    notice = "";
    try {
      await action();
    } catch (caught) {
      error = `${failure} ${errorMessage(caught)}`;
    } finally {
      busy = false;
      support = reminderSupport();
    }
  }

  function turnOn() {
    if (busy) return;
    // The permission request must start inside this tap, before anything else awaits.
    const enabled = enableReminders(id);
    void act(async () => {
      settings = await enabled;
      on = true;
    }, "Could not turn on reminders.");
  }

  function turnOff() {
    void act(async () => {
      settings = await disableReminders(id);
      on = false;
    }, "Could not turn off reminders.");
  }

  function test() {
    void act(async () => {
      await testReminder(id);
      notice = "Test sent.";
    }, "Could not send a test.");
  }

  function saveTime(event: SubmitEvent) {
    event.preventDefault();
    const [hours, minutes] = time.split(":").map(Number);
    void act(async () => {
      settings = await json<ReminderSettings>(`/api/profiles/${id}/reminders`, {
        method: "PUT",
        body: JSON.stringify({ minute_of_day: hours * 60 + minutes }),
      });
      notice = "Reminder time saved.";
    }, "Could not save the reminder time.");
  }
</script>

<section class="reminders" aria-labelledby="reminders-heading">
  <h2 id="reminders-heading">Reminders</h2>
  {#if support === "install"}
    <p>
      On iPhone and iPad, reminders need Keepsake on your Home Screen. In
      Safari, tap Share, then Add to Home Screen. Open Keepsake from its icon
      to turn reminders on.
    </p>
  {:else if support === "insecure"}
    <p>Reminders need Keepsake to be served over HTTPS.</p>
  {:else if support === "unsupported"}
    <p>This browser cannot show reminders.</p>
  {:else if support === "blocked"}
    <p>
      Notifications are blocked for Keepsake. Allow them in your device
      settings, then reload Keepsake.
    </p>
  {:else if settings}
    <p aria-live="polite">
      {on
        ? "Reminders are on for this device."
        : "Reminders are off for this device."}
    </p>
    <div class="actions">
      {#if on}
        <button class="quiet" disabled={busy} onclick={test}>Send a test</button
        ><button class="quiet" disabled={busy} onclick={turnOff}
          >Turn off</button
        >
      {:else}
        <button class="primary" disabled={busy} onclick={turnOn}
          >Turn on reminders</button
        >
      {/if}
    </div>
  {/if}
  {#if settings && (on || settings.devices > 0)}
    <form onsubmit={saveTime}>
      <label
        >Earliest reminder<input
          type="time"
          bind:value={time}
          step="300"
          required
          disabled={busy}
        /></label
      ><button class="quiet" disabled={busy || time === savedTime}
        >Save time</button
      >
    </form>
    <p class="muted">
      At most one reminder a day, once reviews or new facts are ready, before
      midnight in {settings.timezone}.{#if otherDevices > 0}
        {otherDevices === 1
          ? " 1 other device also gets these reminders."
          : ` ${otherDevices} other devices also get these reminders.`}{/if}
    </p>
  {/if}
  {#if error}<p class="message error" role="alert">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
</section>

<style>
  .reminders {
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
  form {
    display: flex;
    align-items: end;
    gap: 0.8rem;
  }
  label {
    display: block;
    flex: 1;
    font-size: 0.9rem;
    font-weight: 600;
  }
  label input {
    margin-top: 0.5rem;
    font-weight: 400;
  }
  .actions {
    display: flex;
    gap: 0.8rem;
    flex-wrap: wrap;
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
