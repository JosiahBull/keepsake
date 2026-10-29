<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "./AppShell.svelte";
  import LibraryApp from "./LibraryApp.svelte";
  import LibraryHeading from "./LibraryHeading.svelte";
  import ProfileSettings from "./ProfileSettings.svelte";
  import Sharing from "./Sharing.svelte";
  import {
    json,
    RequestError,
    errorMessage,
    sharedDraft,
    type FactDraft,
    type Profile,
    type Screen,
  } from "./profiles";
  import { openDialog } from "./dialog";
  import { edgeSwipeBack, iosHomeScreenApp } from "./swipeBack";
  import { preload } from "./preload";
  import { refreshBadge, remindersOn, serviceWorker } from "./pwa";
  import { play, unlockSoundsOnTap } from "./sounds";

  let profiles = $state<Profile[]>([]);
  let active = $state<Profile | null>(null);
  let screen = $state<Screen>("library");
  let loading = $state(true);
  let error = $state("");
  let adding = $state(false);
  let setup = $state<Profile | null>(null);
  let name = $state("");
  let saving = $state(false);
  let pendingProfileCreate = $state<{
    name: string;
    handle: string;
    knownIds: number[];
  } | null>(null);
  let blocked = $state(false);
  let dirty = $state(false);
  let pendingNavigation = $state<(() => void) | null>(null);
  let shareIds = $state<number[]>([]);
  let recipient = $state("");
  let shareSubmission = $state<string | null>(null);
  let sharing = $state(false);
  let shareError = $state("");
  let notice = $state("");
  let chooser = $state(false);
  let mountId = $state(0);
  // Where a Home Screen shortcut or share asked to go once a profile is chosen.
  let pendingScreen = $state<Screen | null>(null);
  let draft = $state<FactDraft | null>(null);
  const activeId = $derived(active?.setup_complete ? active.id : null);
  // Entries Keepsake pushes record how many Keepsake screens lie behind them and
  // which one came last, so back can stay inside Keepsake and say where it goes.
  type Entry = { depth: number; from: Screen };
  let depth = $state(0);
  let cameFrom = $state<Screen | null>(null);
  const titles: Record<Screen, string> = {
    library: "Library",
    sharing: "Library",
    sent: "Library",
    practice: "Practise",
    matching: "Match pairs",
    settings: "Profile settings",
  };

  $effect(() => {
    if (activeId === null) return;
    void remindersOn(activeId).catch(() => undefined);
    void refreshBadge(activeId);
  });

  function remember(id: number) {
    try {
      localStorage.setItem("keepsake.profile", String(id));
    } catch {
      /* URL still works without storage. */
    }
  }

  function requestNavigation(action: () => void) {
    if (blocked || saving || sharing || shareSubmission || pendingProfileCreate)
      return;
    if (dirty) pendingNavigation = action;
    else action();
  }

  function changeProfile(profile: Profile, replace = false) {
    mountId += 1;
    active = profile;
    chooser = false;
    adding = false;
    notice = "";
    error = "";
    dirty = false;
    blocked = false;
    shareIds = [];
    shareSubmission = null;
    if (!profile.setup_complete) {
      setup = profile;
      name = "";
      return;
    }
    setup = null;
    remember(profile.id);
    navigate(pendingScreen ?? "library", replace);
    pendingScreen = null;
  }

  function readEntry() {
    const state: unknown = history.state;
    const entry: Record<string, unknown> =
      typeof state === "object" && state !== null ? { ...state } : {};
    depth =
      typeof entry.depth === "number" && entry.depth > 0 ? entry.depth : 0;
    cameFrom =
      typeof entry.from === "string" && Object.hasOwn(titles, entry.from)
        ? (entry.from as Screen)
        : null;
  }

  function push(path: string) {
    history.pushState({ depth: depth + 1, from: screen } satisfies Entry, "", path);
    readEntry();
  }

  function navigate(next: Screen, replace = false) {
    if (!active) return;
    const path = `/p/${active.id}/${next}`;
    if (location.pathname !== path) {
      if (replace) history.replaceState(history.state, "", path);
      else push(path);
    }
    screen = next;
    window.scrollTo(0, 0);
  }

  // Back goes to the screen before this one, or to the library from a first screen.
  function goBack() {
    if (depth > 0) history.back();
    else requestNavigation(() => navigate("library", true));
  }

  // Guess the first screen from the address so its data loads alongside the profiles.
  function preloadLaunch() {
    const route = /^\/p\/(\d+)\/(library|practice|matching)\/?$/.exec(
      location.pathname,
    );
    if (route) {
      preload(Number(route[1]), route[2] as "library" | "practice" | "matching");
      return;
    }
    if (location.pathname !== "/" && location.pathname !== "/share") return;
    let remembered: string | null = null;
    try {
      remembered = localStorage.getItem("keepsake.profile");
    } catch {
      /* Without storage, the first screen loads after the profiles. */
    }
    if (!remembered || !/^\d+$/.test(remembered)) return;
    const open = new URLSearchParams(location.search).get("open");
    preload(
      Number(remembered),
      location.pathname === "/" && (open === "practice" || open === "matching")
        ? open
        : "library",
    );
  }

  function readLocation() {
    mountId += 1;
    readEntry();
    const match =
      /^\/p\/(\d+)\/(library|practice|matching|sharing|sent|settings)\/?$/.exec(
        location.pathname,
      );
    if (match) {
      const profile = profiles.find((item) => item.id === Number(match[1]));
      if (!profile) {
        active = null;
        chooser = false;
        error = "This profile does not exist. Choose a household profile.";
        return;
      }
      active = profile;
      setup = profile.setup_complete ? null : profile;
      screen = match[2] as Screen;
      pendingScreen = null;
      draft = null;
      remember(profile.id);
      chooser = false;
      return;
    }
    if (location.pathname !== "/" && location.pathname !== "/share") {
      active = null;
      error = "This page does not exist. Choose a household profile.";
      return;
    }
    const params = new URLSearchParams(location.search);
    const open = params.get("open");
    if (location.pathname === "/share") {
      draft = sharedDraft(params);
      pendingScreen = "library";
    } else if (open === "add") {
      draft = { context: "", source: "" };
      pendingScreen = "library";
    } else if (open === "practice" || open === "matching") {
      pendingScreen = open;
    }
    let remembered: string | null = null;
    try {
      remembered = localStorage.getItem("keepsake.profile");
    } catch {
      /* Storage is optional. */
    }
    const profile = profiles.find((item) => String(item.id) === remembered);
    if (profile) changeProfile(profile, true);
    else if (profiles.length === 1 && !profiles[0].setup_complete)
      changeProfile(profiles[0], true);
    else {
      active = null;
      chooser = true;
    }
  }

  onMount(() => {
    let disposed = false;
    void serviceWorker();
    const stopUnlockingSounds = unlockSoundsOnTap();
    preloadLaunch();
    void json<Profile[]>("/api/profiles")
      .then((result) => {
        if (disposed) return;
        profiles = result;
        readLocation();
      })
      .catch(
        (caught) =>
          (error = `Could not load profiles. ${errorMessage(caught)}`),
      )
      .finally(() => (loading = false));
    const pop = () => {
      if (
        blocked ||
        dirty ||
        saving ||
        sharing ||
        shareSubmission ||
        pendingProfileCreate
      ) {
        const requestedPath = location.pathname;
        readEntry();
        if (active) push(`/p/${active.id}/${screen}`);
        requestNavigation(() => {
          push(requestedPath);
          dirty = false;
          readLocation();
        });
        return;
      }
      shareIds = [];
      readLocation();
    };
    const leave = (event: BeforeUnloadEvent) => {
      if (
        blocked ||
        dirty ||
        saving ||
        sharing ||
        shareSubmission ||
        pendingProfileCreate
      ) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    // A notification tapped while Keepsake is open routes here instead of reloading.
    const notificationClick = (event: MessageEvent) => {
      const data: unknown = event.data;
      if (
        typeof data !== "object" ||
        data === null ||
        !("type" in data) ||
        data.type !== "keepsake:navigate" ||
        !("path" in data) ||
        typeof data.path !== "string" ||
        !data.path.startsWith("/")
      )
        return;
      const path = data.path;
      if (location.pathname + location.search === path) return;
      requestNavigation(() => {
        push(path);
        dirty = false;
        shareIds = [];
        readLocation();
      });
    };
    const visibility = () => {
      if (activeId !== null) void refreshBadge(activeId);
    };
    // The Home Screen app has no back button, so an edge swipe goes back as in iOS apps.
    const stopSwipingBack = iosHomeScreenApp()
      ? edgeSwipeBack(
          () => depth > 0 && !blocked,
          () => history.back(),
        )
      : () => undefined;
    window.addEventListener("popstate", pop);
    window.addEventListener("beforeunload", leave);
    document.addEventListener("visibilitychange", visibility);
    if ("serviceWorker" in navigator)
      navigator.serviceWorker.addEventListener("message", notificationClick);
    return () => {
      disposed = true;
      stopUnlockingSounds();
      stopSwipingBack();
      window.removeEventListener("popstate", pop);
      window.removeEventListener("beforeunload", leave);
      document.removeEventListener("visibilitychange", visibility);
      if ("serviceWorker" in navigator)
        navigator.serviceWorker.removeEventListener(
          "message",
          notificationClick,
        );
    };
  });

  function profileHandle(value: string): string {
    const normalized = value
      .normalize("NFKD")
      .replace(/\p{M}/gu, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 30);
    const base = /^[a-z]/.test(normalized)
      ? normalized
      : `profile${normalized ? `-${normalized}` : ""}`;
    const handles = new Set(profiles.map((profile) => profile.handle));
    let candidate = base;
    let suffix = 2;
    while (handles.has(candidate)) candidate = `${base}-${suffix++}`;
    return candidate;
  }

  function finishProfile(profile: Profile) {
    profiles = [
      ...profiles.filter((item) => item.id !== profile.id),
      profile,
    ].sort((a, b) => a.id - b.id);
    pendingProfileCreate = null;
    setup = null;
    adding = false;
    name = "";
    changeProfile(profile);
  }

  async function saveProfile(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    error = "";
    if (!setup)
      pendingProfileCreate ??= {
        name: name.trim(),
        handle: profileHandle(name),
        knownIds: profiles.map((profile) => profile.id),
      };
    try {
      const profile = setup
        ? await json<Profile>(`/api/profiles/${setup.id}`, {
            method: "PATCH",
            body: JSON.stringify({ name: name.trim() }),
          })
        : await json<Profile>("/api/profiles", {
            method: "POST",
            body: JSON.stringify({
              name: pendingProfileCreate!.name,
              handle: pendingProfileCreate!.handle,
            }),
          });
      finishProfile(profile);
    } catch (caught) {
      // A unique stable handle lets a lost creation response be reconciled without making a second library.
      if (pendingProfileCreate) {
        try {
          const refreshed = await json<Profile[]>("/api/profiles");
          const pending = pendingProfileCreate;
          const created = refreshed.find(
            (profile) =>
              profile.handle === pending.handle &&
              profile.name === pending.name &&
              !pending.knownIds.includes(profile.id),
          );
          profiles = refreshed;
          if (created) {
            finishProfile(created);
            return;
          }
        } catch {
          /* Keep the same handle and name for an explicit retry. */
        }
        if (
          caught instanceof RequestError &&
          caught.status >= 400 &&
          caught.status < 500
        )
          pendingProfileCreate = null;
      }
      error = `Could not save the profile. ${errorMessage(caught)}`;
    } finally {
      saving = false;
    }
  }

  // The fact form lives in the library, so open it there.
  function addFact() {
    requestNavigation(() => {
      draft = { context: "", source: "" };
      navigate("library");
    });
  }

  function openShare(ids: number[]) {
    shareIds = [...ids];
    recipient = "";
    shareError = "";
    shareSubmission = null;
  }

  async function sendShare(event: SubmitEvent) {
    event.preventDefault();
    if (!active || sharing) return;
    sharing = true;
    shareError = "";
    shareSubmission ??= crypto.randomUUID();
    try {
      await json(`/api/profiles/${active.id}/shares`, {
        method: "POST",
        body: JSON.stringify({
          submission_id: shareSubmission,
          recipient_id: Number(recipient),
          fact_ids: shareIds,
        }),
      });
      shareIds = [];
      shareSubmission = null;
      notice = "Facts shared.";
      play("send");
      navigate("sent");
    } catch (caught) {
      shareError = `Could not share these facts. ${errorMessage(caught)} Retry with the same selection.`;
      play("alert");
    } finally {
      sharing = false;
    }
  }
</script>

<svelte:head
  ><title>{active ? `${active.name} · Keepsake` : "Keepsake"}</title
  ></svelte:head
>

{#snippet profileMenu()}
  <details class="profile-menu">
    <summary aria-label={`Profile: ${active?.name ?? "Choose profile"}`}
      ><span class="avatar" aria-hidden="true">{[...(active?.name ?? "?")][0]}</span
      ><span class="profile-name">{active?.name ?? "Choose profile"}</span
      ><!-- Glyphs such as ⌄ sit on the text baseline, so the chevron is drawn to centre on the name. --><svg
        class="chevron"
        viewBox="0 0 12 12"
        width="12"
        height="12"
        aria-hidden="true"
        ><path d="M2.5 4.5 6 8l3.5-3.5" /></svg
      ></summary
    >
    <div class="menu">
      <p class="muted">Switch profile</p>
      {#each profiles as profile}
        <button
          class:current={profile.id === active?.id}
          disabled={blocked || saving || sharing || shareSubmission !== null}
          onclick={() => requestNavigation(() => changeProfile(profile))}
          >{profile.name}{profile.id === active?.id ? " ✓" : ""}</button
        >
      {/each}
      <hr />
      <button
        disabled={blocked || saving || shareSubmission !== null}
        onclick={() =>
          requestNavigation(() => {
            adding = true;
            name = "";
          })}>Add profile</button
      >
      <button
        disabled={blocked || saving || shareSubmission !== null}
        onclick={() => requestNavigation(() => navigate("settings"))}
        >Profile settings</button
      >
    </div>
  </details>
{/snippet}

{#if loading}
  <main class="entry"><p role="status">Loading profiles…</p></main>
{:else if setup || adding || !active || chooser}
  <main class="entry">
    <img class="entry-logo" src="/keepsake-logo.png" alt="Keepsake" />
    {#if error}<p class="message error" role="alert">{error}</p>{/if}
    {#if setup || adding}
      <h1>{setup ? "Keep your existing progress" : "Add profile"}</h1>
      {#if setup}<p>
          Your existing library, reviews and schedule will stay with this
          profile.
        </p>{/if}
      <form onsubmit={saveProfile}>
        <label
          >Your name<input
            bind:value={name}
            required
            maxlength="80"
            disabled={saving || pendingProfileCreate !== null}
            autocomplete="given-name"
          /></label
        >
        <div class="actions">
          <button class="primary" disabled={saving}
            >{saving
              ? "Saving…"
              : setup
                ? "Continue"
                : pendingProfileCreate
                  ? "Retry create profile"
                  : "Create profile"}</button
          >{#if !setup}<button
              type="button"
              class="quiet"
              disabled={saving || pendingProfileCreate !== null}
              onclick={() => {
                adding = false;
                error = "";
              }}>Cancel</button
            >{/if}
        </div>
      </form>
    {:else}
      <h1>Who’s practising?</h1>
      <div class="people">
        {#each profiles as profile}<button
            class="person"
            onclick={() => changeProfile(profile)}
            ><span class="initial" aria-hidden="true">{profile.name[0]}</span
            ><strong>{profile.name}</strong></button
          >{/each}
      </div>
      <button
        class="text-button"
        onclick={() => {
          adding = true;
          name = "";
          error = "";
        }}>Add profile</button
      >
      <p class="muted household-note">
        Household profiles have no passwords. Anyone with access can switch
        profiles.
      </p>
    {/if}
  </main>
{:else if ["library", "practice", "matching"].includes(screen)}
  {#key `${active.id}/${mountId}`}
    <LibraryApp
      profile={active}
      initialView={screen as "library" | "practice" | "matching"}
      onNavigate={(next) => {
        if (next === "sharing" || next === "sent")
          requestNavigation(() => navigate(next));
        else navigate(next);
      }}
      {profileMenu}
      {draft}
      onDraftOpened={() => (draft = null)}
      onShare={openShare}
      bind:blocked
      bind:dirty
    />
  {/key}
{:else}
  <AppShell
    title={screen === "settings" ? "Profile settings" : "Library"}
    detail={screen === "settings" ? null : ""}
    back={screen === "settings"
      ? {
          label: depth > 0 && cameFrom ? titles[cameFrom] : "Library",
          onBack: goBack,
        }
      : null}
    current={screen === "settings" ? null : "library"}
    disabled={blocked}
    onNavigate={(next) => requestNavigation(() => navigate(next))}
    onAdd={addFact}
    {profileMenu}
  >
    {#if notice}<p class="message" role="status">{notice}</p>{/if}
    {#if screen === "sharing" || screen === "sent"}
      <LibraryHeading
        current={screen}
        disabled={blocked}
        onNavigate={(next) => requestNavigation(() => navigate(next))}
      />
    {/if}
    {#key `${active.id}/${screen}`}
      {#if screen === "settings"}
        <ProfileSettings
          profile={active}
          bind:blocked
          bind:dirty
          onSaved={(profile) => {
            profiles = profiles.map((item) =>
              item.id === profile.id ? profile : item,
            );
            active = profile;
          }}
          onCancel={() => requestNavigation(() => navigate("library"))}
        />
      {:else}
        <Sharing
          profile={active}
          direction={screen === "sent" ? "sent" : "inbox"}
          bind:blocked
        />
      {/if}
    {/key}
  </AppShell>
{/if}

{#if pendingNavigation}
  <dialog
    class="dialog"
    use:openDialog
    oncancel={(event) => {
      event.preventDefault();
      pendingNavigation = null;
    }}
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="discard-title"
  >
    <h2 id="discard-title">Discard your unsaved answer?</h2>
    <p>
      Your draft will be lost. Saved facts and reviews stay with this profile.
    </p>
    <div class="actions">
      <button class="quiet" onclick={() => (pendingNavigation = null)}
        >Keep working</button
      ><button
        class="primary"
        onclick={() => {
          const action = pendingNavigation;
          pendingNavigation = null;
          dirty = false;
          action?.();
        }}>Discard and continue</button
      >
    </div>
  </dialog>
{/if}
{#if shareIds.length}
  <dialog
    class="dialog"
    use:openDialog
    oncancel={(event) => {
      event.preventDefault();
      if (!shareSubmission && !sharing) shareIds = [];
    }}
    aria-modal="true"
    aria-labelledby="share-title"
  >
    <h2 id="share-title">
      Share {shareIds.length}
      {shareIds.length === 1 ? "fact" : "facts"}
    </h2>
    <form onsubmit={sendShare}>
      <label
        >Share with<select
          aria-label="Share with"
          bind:value={recipient}
          required
          disabled={sharing || shareSubmission !== null}
          ><option value="" disabled>Choose a profile</option
          >{#each profiles.filter((item) => item.id !== active?.id) as profile}<option
              value={profile.id}>{profile.name}</option
            >{/each}</select
        ></label
      >
      <p>
        They can accept copies into their library. Your reviews stay with you;
        later edits stay separate.
      </p>
      {#if profiles.length < 2}<p>
          Add another household profile before sharing.
        </p>{/if}{#if shareError}<p class="message error" role="alert">
          {shareError}
        </p>{/if}
      <div class="actions">
        <button
          type="button"
          class="quiet"
          disabled={sharing || shareSubmission !== null}
          onclick={() => (shareIds = [])}>Cancel</button
        ><button class="primary" disabled={sharing || !recipient}
          >{sharing
            ? "Sharing…"
            : shareSubmission
              ? "Retry share"
              : "Send facts"}</button
        >
      </div>
    </form>
  </dialog>
{/if}

<style>
  :global(select) {
    font: inherit;
    /* iOS zooms into a focused field whose text is smaller than 16px. */
    font-size: 1rem;
    width: 100%;
    color: #18362f;
    background: white;
    border: 1px solid #b4c8c0;
    border-radius: 6px;
    padding: 0.8rem 0.9rem;
  }
  .entry-logo {
    width: 165px;
    display: block;
  }
  .entry {
    max-width: 500px;
    margin: 3rem auto;
    padding: 0 1rem 3rem;
  }
  .entry-logo {
    margin-bottom: 3rem;
  }
  h1 {
    font:
      500 2rem/1.2 "Iowan Old Style",
      Palatino,
      serif;
    letter-spacing: -0.035em;
  }
  .people {
    display: grid;
    gap: 0.8rem;
    margin: 2rem 0 1rem;
  }
  .person {
    display: flex;
    align-items: center;
    gap: 1rem;
    text-align: left;
    background: white;
    color: #18362f;
    border: 1px solid #b4c8c0;
    padding: 1.1rem;
  }
  .initial {
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    background: #e3ede7;
    border-radius: 50%;
  }
  .household-note {
    margin-top: 2rem;
  }
  .profile-menu {
    position: relative;
    margin: 0;
    flex-shrink: 0;
  }
  .profile-menu summary {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: #18362f;
    cursor: pointer;
    border: 1px solid #b4c8c0;
    border-radius: 6px;
    padding: 0.65rem 0.7rem;
    font-weight: 600;
    list-style: none;
    white-space: nowrap;
  }
  .chevron {
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 0.15s;
  }
  .profile-menu[open] .chevron {
    transform: rotate(180deg);
  }
  .avatar {
    display: none;
  }
  .profile-menu summary::-webkit-details-marker {
    display: none;
  }
  .menu {
    position: absolute;
    z-index: 5;
    right: 0;
    top: calc(100% + 0.6rem);
    min-width: 220px;
    background: white;
    padding: 0.5rem;
    border: 1px solid #b4c8c0;
    border-radius: 8px;
    box-shadow: 0 5px 18px #18362f14;
  }
  .menu p {
    margin: 0.5rem 1rem 0.25rem;
  }
  .menu button {
    width: 100%;
    display: block;
    text-align: left;
    background: none;
    color: #244c44;
    padding: 0.8rem 1rem;
  }
  .menu button.current {
    background: #e3ede7;
  }
  .menu hr {
    border: 0;
    border-top: 1px solid #cedbd5;
  }
  form {
    display: grid;
    gap: 1.1rem;
  }
  label {
    font-size: 0.9rem;
    font-weight: 600;
  }
  label input,
  label select {
    display: block;
    margin-top: 0.5rem;
    font-weight: 400;
  }
  .muted {
    color: #64786c;
    font-size: 0.85rem;
    font-weight: 400;
  }
  .primary {
    padding: 0.85rem 1.2rem;
    background: #244c44;
    color: white;
    font-weight: 600;
  }
  .quiet {
    background: transparent;
    color: #244c44;
    padding: 0.8rem 1rem;
    border: 1px solid #c0d0c7;
  }
  .text-button {
    background: none;
    color: #244c44;
    padding: 0.2rem 0;
    text-decoration: underline;
  }
  .actions {
    display: flex;
    gap: 0.8rem;
    flex-wrap: wrap;
  }
  .message {
    background: #e3ede7;
    padding: 1rem 1.2rem;
    border-radius: 6px;
    margin-bottom: 1.5rem;
  }
  .error {
    color: #833d2b;
    background: #f9e9e3;
  }
  /* Shared by every dialog, including the library's delete confirmation. */
  :global(.dialog::backdrop) {
    background: #18362f40;
  }
  :global(.dialog) {
    color: #18362f;
    width: min(100%, 490px);
    max-height: 90vh;
    overflow: auto;
    padding: 2rem;
    background: white;
    border: 1px solid #b4c8c0;
    border-radius: 12px;
  }
  :global(.dialog h2) {
    font:
      500 1.5rem/1.3 "Iowan Old Style",
      Palatino,
      serif;
  }
  /* Phones show the profile as an avatar in the navigation bar. */
  @media (max-width: 700px) {
    .profile-menu summary {
      padding: 0.2rem;
      border: 0;
    }
    .profile-name,
    .chevron {
      display: none;
    }
    .avatar {
      display: grid;
      place-items: center;
      width: 32px;
      height: 32px;
      border-radius: 50%;
      background: #244c44;
      color: #fff;
      font-size: 0.9rem;
    }
    .menu {
      top: calc(100% + 0.35rem);
    }
  }
  @media (max-width: 580px) {
    :global(.dialog) {
      padding: 1.3rem;
    }
  }
</style>
