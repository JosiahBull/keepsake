<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { type components, type paths } from "@keepsake/client";
  import createClient from "openapi-fetch";
  import type { Snippet } from "svelte";
  import type { Profile, Screen } from "./profiles";

  let {
    profile,
    initialView = "library",
    onNavigate,
    profileMenu,
    onShare,
    blocked = $bindable(false),
    dirty = $bindable(false),
  }: {
    profile: Profile;
    initialView?: "library" | "practice" | "matching";
    onNavigate: (view: Screen) => void;
    profileMenu: Snippet;
    onShare: (ids: number[]) => void;
    blocked?: boolean;
    dirty?: boolean;
  } = $props();
  const profileId = untrack(() => profile.id);
  const api = createClient<paths>({ baseUrl: "/" });
  const controller = new AbortController();
  api.use({
    async onRequest({ request }) {
      const url = new URL(request.url);
      url.pathname = `/api/profiles/${profileId}${url.pathname.slice(4)}`;
      const body = request.body ? await request.arrayBuffer() : undefined;
      return new Request(url, {
        method: request.method,
        headers: request.headers,
        body,
        signal: controller.signal,
      });
    },
  });

  type Method = components["schemas"]["PracticeKind"];
  type Outcome = components["schemas"]["ReviewOutcome"];
  type Fact = components["schemas"]["FactSummary"];
  type Question = components["schemas"]["PracticeQuestion"];
  type StudyQuestion = components["schemas"]["StudyQuestion"];
  type StudyQueue = components["schemas"]["StudyQueue"];
  type StudySubmission = components["schemas"]["StudySubmission"];
  type StudyResult = components["schemas"]["StudyResult"];
  type ExposureSubmission = components["schemas"]["ExposureSubmission"];
  type PendingExposure = {
    body: ExposureSubmission;
    error: string;
    busy: boolean;
  };

  const modes: { id: Method; name: string; instruction: string }[] = [
    {
      id: "flashcard",
      name: "Flashcards",
      instruction:
        "Recall the answer in your own words before turning the card.",
    },
    {
      id: "multiple_choice",
      name: "Multiple choice",
      instruction: "Choose the best answer, then check the feedback.",
    },
    {
      id: "cloze",
      name: "Fill the gap",
      instruction:
        "Recall the missing information before checking the complete answer.",
    },
    {
      id: "explain",
      name: "Explain it",
      instruction:
        "Explain the relationship in your own words, then compare the key points.",
    },
    {
      id: "apply",
      name: "Apply it",
      instruction:
        "Work through the situation before checking the example answer.",
    },
  ];

  const ratings: { id: Outcome; name: string; detail: string }[] = [
    { id: "again", name: "Again", detail: "Could not recall" },
    { id: "hard", name: "Hard", detail: "Correct, with difficulty" },
    { id: "good", name: "Good", detail: "Correct" },
    { id: "easy", name: "Easy", detail: "Correct, without effort" },
  ];

  // Library and navigation
  let facts = $state<Fact[]>([]);
  let loading = $state(true);
  let error = $state("");
  let notice = $state("");
  let view = $state<"library" | "practice" | "matching">(
    untrack(() => initialView),
  );
  let selectedFacts = $state<number[]>([]);
  let search = $state("");
  let totalFacts = $state(0);
  let nextCursor = $state<number | null>(null);
  let beforeCursor = $state<number | null>(null);
  let pageHistory = $state<(number | null)[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let libraryRequest = 0;
  let practiceRequest = 0;
  let matchingRequest = 0;

  // Fact form
  let showAdd = $state(false);
  let saving = $state(false);
  let prompt = $state("");
  let answer = $state("");
  let context = $state("");
  let source = $state("");
  let tags = $state("");

  // Practice session
  let queue = $state<(Question | StudyQuestion)[]>([]);
  let practiceMode = $state<"scheduled" | "extra">("scheduled");
  let studyQueue = $state<StudyQueue | null>(null);
  let recalled = $state(false);
  let pendingSubmission = $state<StudySubmission | null>(null);
  let feedbackEventId = $state<number | null>(null);
  let studyConflict = $state(false);
  let exposureOnly = $state(false);
  let exposures = $state<PendingExposure[]>([]);
  let matchingExposureId = $state<string | null>(null);
  const locallyExposed = new Map<number, number>();
  let practiceLoading = $state(false);
  let practiceError = $state("");
  let practiceTotal = $state(0);
  let cursor = $state(0);
  let revealed = $state(false);
  let revealRequested = $state(false);
  let pendingAnswerExposureId = $state<string | null>(null);
  let choiceExposureId = $state<string | null>(null);
  let choicesReady = $state(false);
  let disclosedFacts = $state<number[]>([]);
  const disclosureRequests = new Map<number, string>();
  let response = $state("");
  let choice = $state<string | null>(null);
  let startedAt = $state(0);
  let responseMs = $state(0);
  let sessionCount = $state(0);
  let reviewError = $state("");
  let recording = $state(false);

  // Matching round
  let matchFacts = $state<Fact[]>([]);
  let matchingLoading = $state(false);
  let matchingError = $state("");
  let matchAnswers = $state<Fact[]>([]);
  let matched = $state<number[]>([]);
  let selectedPrompt = $state<number | null>(null);
  let selectedAnswer = $state<number | null>(null);
  let matchFeedback = $state("");
  let pendingMatch = $state<number | null>(null);
  let matchStarted = $state<Record<number, number>>({});
  let matchMs = $state(0);
  let matchMisses = $state<Record<number, number>>({});

  // Derived library and practice data
  const current = $derived(queue[cursor]);
  const mode = $derived(current?.practice.kind);
  const choiceOptions = $derived(
    current ? shuffled(current.practice.choices) : [],
  );
  const selectedMode = $derived(modes.find((item) => item.id === mode));
  const pendingFact = $derived(
    matchFacts.find((fact) => fact.id === pendingMatch),
  );
  const scheduledQuestion = $derived(
    practiceMode === "scheduled" && current && "target_id" in current
      ? current
      : null,
  );
  const exposing = $derived(exposures.some((exposure) => exposure.busy));
  const navigationBlocked = $derived(
    recording || exposing || pendingSubmission !== null || saving,
  );
  const matchingBlocked = $derived(
    exposures.some(
      (exposure) => exposure.body.submission_id === matchingExposureId,
    ),
  );
  const answerExposurePending = $derived(
    Boolean(
      current &&
      exposures.some((exposure) =>
        exposure.body.fact_ids.includes(current.fact.id),
      ),
    ),
  );

  $effect(() => {
    blocked =
      navigationBlocked ||
      exposures.length > 0 ||
      Boolean(reviewError && !studyConflict);
    dirty = Boolean(
      response.trim() ||
      prompt.trim() ||
      answer.trim() ||
      context.trim() ||
      source.trim() ||
      tags.trim(),
    );
  });

  onMount(() => {
    if (initialView === "practice") void startPractice();
    else if (initialView === "matching") void startMatching();
    else void loadFacts();
    return () => {
      clearTimeout(searchTimer);
      controller.abort();
      libraryRequest += 1;
      practiceRequest += 1;
      matchingRequest += 1;
    };
  });

  // Library and fact form actions
  async function loadFacts(
    cursor: number | null = beforeCursor,
    history = pageHistory,
  ) {
    const request = ++libraryRequest;
    loading = true;
    error = "";
    beforeCursor = cursor;
    pageHistory = [...history];
    facts = [];
    nextCursor = null;

    try {
      const page = unwrap(
        await api.GET("/api/facts", {
          params: {
            query: {
              limit: 25,
              ...(cursor === null ? {} : { before_id: cursor }),
              search,
            },
          },
        }),
      );

      if (request !== libraryRequest) {
        return;
      }

      facts = page.items;
      totalFacts = page.total;
      nextCursor = page.next_cursor ?? null;
    } catch (caught) {
      if (request === libraryRequest) {
        error = `Could not load your library. ${message(caught)}`;
      }
    } finally {
      if (request === libraryRequest) {
        loading = false;
      }
    }
  }

  function searchLibrary(event: Event) {
    search = (event.currentTarget as HTMLInputElement).value;
    clearTimeout(searchTimer);

    // Invalidate an earlier search immediately, before the debounce expires.
    libraryRequest += 1;
    searchTimer = setTimeout(() => {
      void loadFacts(null, []);
    }, 200);
  }

  function focusPageContent() {
    if (!error) {
      document.querySelector<HTMLElement>(".fact-details > summary")?.focus();
    }
  }

  async function previousPage() {
    if (pageHistory.length === 0 || loading) {
      return;
    }

    await loadFacts(pageHistory.at(-1) ?? null, pageHistory.slice(0, -1));
    focusPageContent();
  }

  async function nextPage() {
    if (nextCursor === null || loading) {
      return;
    }

    await loadFacts(nextCursor, [...pageHistory, beforeCursor]);
    focusPageContent();
  }

  function showLibrary() {
    if (!navigationBlocked) {
      view = "library";
      onNavigate("library");
    }
  }

  function openAdd() {
    if (!navigationBlocked) {
      view = "library";
      onNavigate("library");
      showAdd = !showAdd;
    }
  }

  async function addFact(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    error = "";
    notice = "";

    try {
      for (const [label, value, limit] of [
        ["Question", prompt, 8000],
        ["Answer", answer, 32000],
        ["Context", context, 64000],
        ["Source", source, 4000],
      ] as const) {
        if (new TextEncoder().encode(value.trim()).length > limit) {
          throw new Error(`${label} is too long. Shorten it before saving.`);
        }
      }

      unwrap(
        await api.POST("/api/facts", {
          body: {
            prompt: prompt.trim(),
            answer: answer.trim(),
            context: context.trim(),
            source: source.trim(),
            tags: tags
              .split(",")
              .map((tag) => tag.trim())
              .filter(Boolean),
            practices: [],
          },
        }),
      );

      showAdd = false;
      prompt = "";
      answer = "";
      context = "";
      source = "";
      tags = "";
      notice = "Fact saved.";
      search = "";
      clearTimeout(searchTimer);
      await loadFacts(null, []);
    } catch (caught) {
      error = `Could not save the fact. ${message(caught)}`;
    } finally {
      saving = false;
    }
  }

  // Practice actions
  function resetQuestion() {
    revealed = false;
    revealRequested = false;
    pendingAnswerExposureId = null;
    choiceExposureId = null;
    choicesReady = false;
    response = "";
    choice = null;
    startedAt = Date.now();
    responseMs = 0;
    reviewError = "";
    recalled = false;
    pendingSubmission = null;
    feedbackEventId = null;
    studyConflict = false;
    exposureOnly = false;
    if (
      practiceMode === "extra" &&
      current?.practice.kind === "multiple_choice"
    ) {
      void exposeChoices(current);
    }
  }

  async function exposeChoices(question: Question) {
    const id = await recordExposure([question.fact.id], "multiple_choice");
    if (current?.practice.id !== question.practice.id) {
      return;
    }

    choiceExposureId = id;
    choicesReady = !exposures.some((entry) => entry.body.submission_id === id);
  }

  async function startPractice(extra = false) {
    if (navigationBlocked) {
      return;
    }

    if (!extra && exposures.length) {
      view = "practice";
      practiceError = "Save the pending activity before starting practice.";
      return;
    }

    const request = ++practiceRequest;
    view = "practice";
    onNavigate("practice");
    practiceLoading = true;
    practiceError = "";
    practiceMode = extra ? "extra" : "scheduled";
    studyQueue = null;
    queue = [];
    cursor = 0;
    sessionCount = 0;
    resetQuestion();

    try {
      const session = extra
        ? unwrap(
            await api.GET("/api/practice", {
              params: { query: { limit: 20 } },
            }),
          )
        : unwrap(
            await api.GET("/api/study/queue", {
              params: { query: { limit: 20 } },
            }),
          );

      if (request !== practiceRequest) {
        return;
      }

      queue = session.items;
      if ("total_due" in session) {
        studyQueue = session;
        practiceTotal = session.total_due + session.new_remaining;
      } else {
        practiceTotal = session.total;
      }
      resetQuestion();
    } catch (caught) {
      if (request === practiceRequest) {
        practiceError = `Could not load practice. ${message(caught)}`;
      }
    } finally {
      if (request === practiceRequest) {
        practiceLoading = false;
      }
    }
  }

  async function reveal() {
    if (revealRequested || recording) {
      return;
    }

    responseMs = Math.max(0, Date.now() - startedAt);
    revealRequested = true;
    if (scheduledQuestion) {
      exposureOnly = !recalled && !response.trim();
      pendingSubmission = studySubmission(null, "exposure");
      await saveStudySubmission(false);
    } else if (!scheduledQuestion && current) {
      pendingAnswerExposureId = await recordExposure(
        [current.fact.id],
        current.practice.kind,
      );
      if (
        !exposures.some(
          (entry) => entry.body.submission_id === pendingAnswerExposureId,
        )
      ) {
        revealed = true;
      }
    }
  }

  function studySubmission(
    outcome: Outcome | null,
    kind: "attempt" | "exposure",
  ): StudySubmission {
    if (!scheduledQuestion) {
      throw new Error("No scheduled question is available.");
    }
    return {
      submission_id: crypto.randomUUID(),
      target_id: scheduledQuestion.target_id,
      practice_id: scheduledQuestion.practice.id,
      content_revision: scheduledQuestion.content_revision,
      expected_version: scheduledQuestion.schedule_version,
      kind,
      context: "scheduled",
      method: scheduledQuestion.practice.kind,
      outcome,
      response_ms: responseMs,
      attempted: recalled || Boolean(response.trim()),
      assisted: false,
      prior_exposure:
        Date.now() -
          (locallyExposed.get(scheduledQuestion.fact.id) ?? -Infinity) <
        600_000,
      ...(kind === "attempt" && feedbackEventId !== null
        ? { feedback_event_id: feedbackEventId }
        : {}),
    };
  }

  async function saveStudySubmission(
    advance = pendingSubmission?.kind === "attempt",
  ) {
    if (!pendingSubmission || recording) {
      return;
    }
    recording = true;
    reviewError = "";
    try {
      const response = await api.POST("/api/study/events", {
        body: pendingSubmission,
      });
      if (response.response.status === 409) {
        studyConflict = true;
        pendingSubmission = null;
        reviewError = "This attempt is no longer current. Refresh practice.";
        return;
      }
      const result: StudyResult = unwrap(response);
      if (
        pendingSubmission.kind === "exposure" &&
        pendingSubmission.attempted
      ) {
        feedbackEventId = result.event_id;
      }
      if (pendingSubmission.kind === "exposure") {
        revealed = true;
      }
      if (
        scheduledQuestion &&
        result.schedule.target_id === scheduledQuestion.target_id
      ) {
        scheduledQuestion.schedule_version = result.schedule.version;
      }
      if (studyQueue) {
        studyQueue = { ...studyQueue, next_due_at: result.next_due_at };
      }
      pendingSubmission = null;

      if (advance) {
        sessionCount += 1;
        cursor += 1;
        resetQuestion();
        if (cursor >= queue.length) {
          await refreshStudyOverview();
        }
      }
    } catch (caught) {
      const action =
        pendingSubmission?.kind === "exposure"
          ? "Could not save this activity."
          : "Could not save this attempt.";
      reviewError = `${action} ${message(caught)}`;
    } finally {
      recording = false;
    }
  }

  async function continueAfterExposure() {
    if (recording || pendingSubmission || studyConflict) {
      return;
    }

    cursor += 1;
    resetQuestion();
    if (cursor >= queue.length) {
      await refreshStudyOverview();
    }
  }

  async function refreshStudyOverview() {
    try {
      studyQueue = unwrap(
        await api.GET("/api/study/queue", { params: { query: { limit: 20 } } }),
      );
    } catch (caught) {
      practiceError = `Could not check the next review. ${message(caught)}`;
    }
  }

  function intervalLabel(seconds: number): string {
    if (seconds < 3600) {
      return `${Math.round(seconds / 60)} min`;
    }
    if (seconds < 86_400) {
      return `${Math.round(seconds / 3600)} hr`;
    }

    const days = Math.round(seconds / 86_400);
    return `${days} ${days === 1 ? "day" : "days"}`;
  }

  function dueLabel(timestamp: number): string {
    return new Intl.DateTimeFormat(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
      timeZoneName: "short",
      timeZone: studyQueue?.timezone ?? "Europe/London",
    }).format(new Date(timestamp * 1000));
  }

  async function saveExposure(submissionId: string) {
    const exposure = exposures.find(
      (entry) => entry.body.submission_id === submissionId,
    );
    if (!exposure || exposure.busy) {
      return;
    }

    exposures = exposures.map((entry) =>
      entry === exposure ? { ...entry, busy: true, error: "" } : entry,
    );
    try {
      unwrap(await api.POST("/api/study/exposures", { body: exposure.body }));
      const authorizedFacts = exposure.body.fact_ids.filter(
        (id) => disclosureRequests.get(id) === submissionId,
      );
      disclosedFacts = [...new Set([...disclosedFacts, ...authorizedFacts])];
      exposures = exposures.filter(
        (entry) => entry.body.submission_id !== submissionId,
      );
      if (pendingAnswerExposureId === submissionId) {
        revealed = true;
      }
      if (choiceExposureId === submissionId) {
        choicesReady = true;
      }
    } catch (caught) {
      exposures = exposures.map((entry) =>
        entry.body.submission_id === submissionId
          ? { ...entry, busy: false, error: message(caught) }
          : entry,
      );
    }
  }

  async function recordExposure(
    factIds: number[],
    method: Method | "matching",
    disclosureFactId?: number,
  ) {
    const body: ExposureSubmission = {
      submission_id: crypto.randomUUID(),
      fact_ids: factIds,
      method,
    };
    factIds.forEach((id) => locallyExposed.set(id, Date.now()));
    if (disclosureFactId !== undefined) {
      disclosureRequests.set(disclosureFactId, body.submission_id);
    }
    exposures = [...exposures, { body, error: "", busy: false }];
    await saveExposure(body.submission_id);
    return body.submission_id;
  }

  function discloseFact(event: Event, factId: number) {
    if ((event.currentTarget as HTMLDetailsElement).open) {
      void recordExposure([factId], "flashcard", factId);
    } else {
      clearDisclosure(factId);
    }
  }

  function clearDisclosure(factId: number) {
    disclosedFacts = disclosedFacts.filter((id) => id !== factId);
    disclosureRequests.delete(factId);
  }

  async function recordReview(
    factId: number,
    practiceId: number | null,
    method: Method | "matching",
    outcome: Outcome,
    elapsed: number,
  ) {
    unwrap(
      await api.POST("/api/reviews", {
        body: {
          fact_id: factId,
          practice_id: practiceId,
          method,
          outcome,
          response_ms: elapsed,
        },
      }),
    );
  }

  async function grade(outcome: Outcome) {
    if (!current || recording || answerExposurePending) {
      return;
    }

    if (scheduledQuestion) {
      if (exposureOnly || studyConflict || pendingSubmission) {
        return;
      }
      pendingSubmission = studySubmission(outcome, "attempt");
      await saveStudySubmission();
      return;
    }

    recording = true;
    reviewError = "";

    try {
      await recordReview(
        current.fact.id,
        current.practice.id,
        current.practice.kind,
        outcome,
        responseMs,
      );
      cursor += 1;
      sessionCount += 1;
      resetQuestion();
    } catch (caught) {
      reviewError = `Review was not saved. ${message(caught)} Try again.`;
    } finally {
      recording = false;
    }
  }

  // Matching actions
  async function startMatching() {
    if (navigationBlocked) {
      return;
    }

    const request = ++matchingRequest;
    view = "matching";
    onNavigate("matching");
    matchingLoading = true;
    matchingError = "";
    matchFacts = [];
    matchAnswers = [];
    matched = [];
    selectedPrompt = null;
    selectedAnswer = null;
    pendingMatch = null;
    matchFeedback = "";
    matchStarted = {};
    matchMisses = {};
    reviewError = "";

    try {
      const sample = unwrap(
        await api.GET("/api/facts/sample", {
          params: { query: { limit: 4 } },
        }),
      );

      if (request !== matchingRequest) {
        return;
      }

      matchFacts = sample.items;
      matchAnswers = shuffled(matchFacts);
      if (matchFacts.length >= 3) {
        matchingExposureId = await recordExposure(
          matchFacts.map((fact) => fact.id),
          "matching",
        );
      }
    } catch (caught) {
      if (request === matchingRequest) {
        matchingError = `Could not load matching pairs. ${message(caught)}`;
      }
    } finally {
      if (request === matchingRequest) {
        matchingLoading = false;
      }
    }
  }

  function choosePrompt(id: number) {
    if (pendingMatch !== null || recording || matchingBlocked) {
      return;
    }

    selectedPrompt = id;
    selectedAnswer = null;
    matchFeedback = "";
    if (!matchStarted[id]) {
      matchStarted = { ...matchStarted, [id]: Date.now() };
    }
  }

  function chooseAnswer(id: number) {
    if (selectedPrompt === null || pendingMatch !== null || recording) {
      return;
    }

    selectedAnswer = id;
    if (id === selectedPrompt) {
      pendingMatch = id;
      matchMs = Math.max(0, Date.now() - matchStarted[id]);
      matchFeedback = "Matched. Rate your attempt.";
    } else {
      matchMisses = {
        ...matchMisses,
        [selectedPrompt]: (matchMisses[selectedPrompt] || 0) + 1,
      };
      matchFeedback = "Not a match. Try another answer.";
    }
  }

  async function gradeMatch(outcome: Outcome) {
    if (pendingMatch === null || recording) {
      return;
    }

    const id = pendingMatch;
    recording = true;
    reviewError = "";

    try {
      await recordReview(id, null, "matching", outcome, matchMs);
      matched = [...matched, id];
      pendingMatch = null;
      selectedPrompt = null;
      selectedAnswer = null;
      matchFeedback =
        matched.length === matchFacts.length
          ? "Round complete."
          : "Choose another question.";
    } catch (caught) {
      reviewError = `Review was not saved. ${message(caught)} Try the rating again.`;
    } finally {
      recording = false;
    }
  }

  // Shared helpers
  function unwrap<T>(result: {
    data?: T;
    error?: components["schemas"]["ErrorBody"];
  }): T {
    if (result.error) {
      throw new Error(result.error.error);
    }
    if (result.data === undefined) {
      throw new Error("The server returned no data.");
    }

    return result.data;
  }

  function message(caught: unknown) {
    return caught instanceof Error
      ? caught.message
      : "Something went wrong. Try again.";
  }

  function shuffled<T>(items: T[]) {
    const result = [...items];

    for (let index = result.length - 1; index > 0; index--) {
      const other = Math.floor(Math.random() * (index + 1));
      [result[index], result[other]] = [result[other], result[index]];
    }

    return result;
  }

  function sourceUrl(value: string): boolean {
    try {
      return ["https:", "http:"].includes(new URL(value).protocol);
    } catch {
      return false;
    }
  }
</script>

<svelte:head><title>Keepsake</title></svelte:head>

<div class="app-shell">
  <header class="app-header">
    <a
      class="brand"
      href="#library"
      onclick={showLibrary}
      aria-label="Keepsake library"
    >
      <img src="/keepsake-logo.png" alt="Keepsake" width="2172" height="724" />
    </a>
    <nav aria-label="Main navigation">
      <button
        class:active={view === "library"}
        disabled={navigationBlocked}
        onclick={showLibrary}
      >
        Library
      </button>
      <button
        class:active={view === "practice"}
        disabled={navigationBlocked}
        onclick={() => startPractice()}
      >
        Practise
      </button>
      <button
        class:active={view === "matching"}
        disabled={navigationBlocked}
        onclick={startMatching}
      >
        Match pairs
      </button>
    </nav>
    <button
      class="primary add-top"
      disabled={navigationBlocked}
      onclick={openAdd}
    >
      {showAdd ? "Close form" : "Add a fact"}
    </button>
    {@render profileMenu()}
  </header>

  <main>
    {#if exposing}
      <p role="status">Saving…</p>
    {/if}
    {#each exposures.filter((entry) => entry.error) as exposure (exposure.body.submission_id)}
      <div class="message error" role="alert">
        Could not save this activity. {exposure.error}
        <button
          class="text-button"
          disabled={exposure.busy}
          onclick={() => saveExposure(exposure.body.submission_id)}
          >Retry save</button
        >
      </div>
    {/each}
    {#if error && view === "library"}
      <div class="message error" role="alert">
        {error}

        {#if !showAdd}
          <button class="text-button" onclick={() => loadFacts()}
            >Try again</button
          >
        {/if}
      </div>
    {/if}

    {#if notice}
      <p class="message success" role="status">{notice}</p>
    {/if}

    {#if view === "library"}
      <div class="page-heading">
        <h1>Library</h1>
        <button
          class="primary mobile-add"
          disabled={navigationBlocked}
          onclick={openAdd}>{showAdd ? "Close form" : "Add a fact"}</button
        >

        {#if !error && !loading}
          <span class="muted">
            {totalFacts}
            {totalFacts === 1 ? "fact" : "facts"}
          </span>
        {/if}
      </div>

      <div class="library-tabs">
        <button class="active" disabled>My library</button><button
          disabled={blocked}
          onclick={() => onNavigate("sharing")}>Shared with me</button
        ><button disabled={blocked} onclick={() => onNavigate("sent")}
          >Sent facts</button
        >
      </div>

      {#if selectedFacts.length}<div class="share-selection">
          <span>{selectedFacts.length} selected</span><button
            class="quiet"
            disabled={blocked}
            onclick={() => onShare(selectedFacts)}>Share selected</button
          ><button class="text-button" onclick={() => (selectedFacts = [])}
            >Clear selection</button
          >
        </div>{/if}

      {#if showAdd}
        <section class="add-panel" aria-labelledby="add-heading">
          <h2 id="add-heading">Add a fact</h2>
          <form onsubmit={addFact}>
            <label>
              Question
              <textarea bind:value={prompt} required maxlength="8000" rows="2"
              ></textarea>
            </label>
            <label>
              Answer
              <textarea bind:value={answer} required maxlength="20000" rows="3"
              ></textarea>
            </label>
            <details class="form-details">
              <summary>Context, source and tags (optional)</summary>
              <div class="optional-fields">
                <label>
                  Context
                  <textarea
                    bind:value={context}
                    maxlength="30000"
                    rows="2"
                    placeholder="Background or qualifications"
                  ></textarea>
                </label>
                <div class="form-pair">
                  <label>
                    Source
                    <input
                      bind:value={source}
                      maxlength="2000"
                      placeholder="URL, book or document"
                    />
                  </label>
                  <label>
                    Tags <span class="optional">comma separated</span>
                    <input bind:value={tags} maxlength="1000" />
                  </label>
                </div>
              </div>
            </details>
            <div class="form-actions">
              <button type="submit" class="primary" disabled={saving}>
                {saving ? "Saving…" : "Save fact"}
              </button>
              <button
                type="button"
                class="quiet"
                onclick={() => {
                  showAdd = false;
                }}
              >
                Cancel
              </button>
            </div>
          </form>
        </section>
      {/if}

      {#if totalFacts > 0 || search || pageHistory.length > 0}
        <div class="library-tools">
          <label class="search">
            <span class="sr-only">Search facts</span>
            <input
              type="search"
              value={search}
              oninput={searchLibrary}
              placeholder="Search facts or tags"
              maxlength="1000"
            />
          </label>
        </div>
      {/if}
      {#if loading}
        <p role="status">Loading library…</p>
      {:else if totalFacts === 0 && !search}
        {#if !showAdd && !error}
          <p class="empty-state">No facts yet. Add a fact to start.</p>
        {/if}
      {:else}
        <section class="fact-list" aria-label="Saved facts">
          {#each facts as fact (fact.id)}
            <article class="fact-row">
              <input
                class="fact-select"
                type="checkbox"
                aria-label={`Select ${fact.prompt}`}
                checked={selectedFacts.includes(fact.id)}
                disabled={!selectedFacts.includes(fact.id) &&
                  selectedFacts.length >= 20}
                onchange={() =>
                  (selectedFacts = selectedFacts.includes(fact.id)
                    ? selectedFacts.filter((id) => id !== fact.id)
                    : [...selectedFacts, fact.id])}
              />
              <details
                class="fact-details"
                ontoggle={(event) => discloseFact(event, fact.id)}
              >
                <summary onclick={() => clearDisclosure(fact.id)}>
                  <h2>{fact.prompt}</h2>
                </summary>
                {#if disclosedFacts.includes(fact.id)}
                  <div class="fact-body">
                    <p class="answer-copy">{fact.answer}</p>

                    {#if fact.context}
                      <p class="context-copy">{fact.context}</p>
                    {/if}

                    {#if fact.source}
                      <p class="source">
                        Source:
                        {#if sourceUrl(fact.source)}
                          <a
                            href={fact.source}
                            title={fact.source}
                            target="_blank"
                            rel="noreferrer"
                          >
                            {fact.source}
                          </a>
                        {:else}
                          {fact.source}
                        {/if}
                      </p>
                    {/if}

                    {#if fact.shared_by}<p class="source">
                        Shared by {fact.shared_by}
                      </p>{/if}

                    {#if fact.tags.length}
                      <p class="tags">Tags: {fact.tags.join(", ")}</p>
                    {/if}
                  </div>
                {/if}
              </details>
            </article>
          {:else}
            {#if !error}
              <p class="no-results">
                No facts match “{search}”. Clear or change your search.
              </p>
            {/if}
          {/each}
        </section>
        {#if pageHistory.length > 0 || nextCursor !== null}
          <nav class="pagination" aria-label="Library pages">
            <button
              class="quiet"
              disabled={loading || pageHistory.length === 0}
              onclick={previousPage}>Previous</button
            >
            <span class="muted">Page {pageHistory.length + 1}</span>
            <button
              class="quiet"
              disabled={loading || nextCursor === null}
              onclick={nextPage}>Next</button
            >
          </nav>
        {/if}
      {/if}
    {:else if view === "practice"}
      <h1 class="sr-only">Practise</h1>
      <div class="practice-layout">
        <div class="practice-toolbar">
          <span class="muted"
            >{practiceMode === "scheduled"
              ? "Scheduled recall"
              : "Extra practice · does not advance recall"}</span
          >
          <button
            class="text-button"
            disabled={navigationBlocked}
            onclick={() => startPractice(practiceMode === "scheduled")}
          >
            {practiceMode === "scheduled"
              ? "Extra practice"
              : "Scheduled recall"}
          </button>
        </div>
        {#if studyQueue?.budget_reached}
          <p class="message" role="status">
            Daily study budget reached. You can stop here or keep going.
          </p>
        {/if}
        <section
          class="practice-stage"
          aria-label={selectedMode?.name ?? "Practice session"}
        >
          {#if practiceLoading}
            <p role="status">Loading practice…</p>
          {:else if practiceError}
            <div class="message error" role="alert">{practiceError}</div>
            <button
              class="quiet"
              onclick={() => startPractice(practiceMode === "extra")}
              >Try again</button
            >
          {:else if current}
            <p class="card-meta">
              {selectedMode?.name} · {cursor + 1} of {queue.length}
            </p>
            <h2 class="question">{current.practice.prompt}</h2>

            {#if mode === "multiple_choice"}
              {#if choicesReady}
                <fieldset
                  class="choices"
                  disabled={revealed || revealRequested}
                >
                  <legend class="sr-only">Choose an answer</legend>

                  {#each choiceOptions as option}
                    <label
                      class:chosen={choice === option}
                      class:correct={revealed &&
                        option === current.practice.answer}
                      class:incorrect={revealed &&
                        choice === option &&
                        option !== current.practice.answer}
                    >
                      <input
                        type="radio"
                        name="answer"
                        value={option}
                        bind:group={choice}
                      />
                      <span>{option}</span>
                    </label>
                  {/each}
                </fieldset>
              {/if}
            {:else}
              <label class="response-label">
                Your answer <span class="optional">optional</span>
                <textarea
                  rows={mode === "flashcard" || mode === "cloze" ? 3 : 5}
                  bind:value={response}
                  readonly={revealed || revealRequested}
                  placeholder="Write or say your answer before revealing."
                ></textarea>
              </label>
              {#if scheduledQuestion && !revealRequested}
                <label class="recall-confirmation">
                  <input type="checkbox" bind:checked={recalled} />
                  I tried to recall
                </label>
              {/if}
            {/if}

            {#if !revealed}
              <button
                class="primary"
                disabled={recording ||
                  revealRequested ||
                  (mode === "multiple_choice" && choice === null)}
                onclick={reveal}
              >
                {mode === "multiple_choice" ? "Check answer" : "Reveal answer"}
              </button>
            {:else}
              <div class="answer-panel">
                <h3>
                  {mode === "multiple_choice"
                    ? choice === current.practice.answer
                      ? "Correct"
                      : "Correct answer"
                    : "Answer"}
                </h3>
                <p class="answer-copy">{current.practice.answer}</p>

                {#if current.practice.explanation}
                  <p class="context-copy">{current.practice.explanation}</p>
                {/if}

                {#if current.fact.context && current.fact.context !== current.practice.explanation}
                  <details>
                    <summary>Context</summary>
                    <p class="context-copy">{current.fact.context}</p>
                  </details>
                {/if}

                {#if current.fact.source}
                  <p class="source">
                    Source:
                    {#if sourceUrl(current.fact.source)}
                      <a
                        href={current.fact.source}
                        title={current.fact.source}
                        target="_blank"
                        rel="noreferrer"
                      >
                        {current.fact.source}
                      </a>
                    {:else}
                      {current.fact.source}
                    {/if}
                  </p>
                {/if}

                {#if practiceMode === "extra"}
                  <p class="verification-note">
                    Check generated answers against their source.
                  </p>
                {/if}
              </div>
              <div class="grade-panel">
                {#if exposureOnly}
                  <p>
                    Answer viewed without a recall attempt. This does not
                    advance its schedule.
                  </p>
                  <button
                    class="primary"
                    disabled={recording ||
                      pendingSubmission !== null ||
                      studyConflict}
                    onclick={continueAfterExposure}>Continue</button
                  >
                {:else}
                  <h3>Rate your attempt</h3>
                  <div class="ratings">
                    {#each ratings as rating}
                      <button
                        class="rating"
                        disabled={recording ||
                          pendingSubmission !== null ||
                          studyConflict ||
                          answerExposurePending}
                        onclick={() => grade(rating.id)}
                      >
                        <strong>{rating.name}</strong>
                        <span>{rating.detail}</span>
                        {#if scheduledQuestion}<span
                            >{intervalLabel(
                              scheduledQuestion.intervals[rating.id],
                            )}</span
                          >{/if}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
            {#if reviewError}
              <p class="error inline-error" role="alert">{reviewError}</p>
              {#if studyConflict}
                <button class="quiet" onclick={() => startPractice()}
                  >Refresh practice</button
                >
              {:else if pendingSubmission}
                <button
                  class="quiet"
                  disabled={recording}
                  onclick={() => saveStudySubmission()}>Retry save</button
                >
              {/if}
            {/if}
            {#if recording}
              <p role="status">
                {pendingSubmission?.kind === "exposure"
                  ? "Saving…"
                  : "Saving review…"}
              </p>
            {/if}
          {:else if practiceMode === "scheduled"}
            <div class="session-done">
              <h2>
                {sessionCount > 0
                  ? "Session complete"
                  : "Nothing due right now"}
              </h2>
              {#if sessionCount > 0}<p>
                  {sessionCount}
                  {sessionCount === 1 ? "review" : "reviews"} saved.
                </p>{/if}
              {#if studyQueue?.next_due_at}<p>
                  Next review: {dueLabel(studyQueue.next_due_at)}.
                </p>{/if}
              {#if studyQueue && studyQueue.items.length > 0}
                <button class="primary" onclick={() => startPractice()}
                  >Continue practice</button
                >
              {/if}
              <button class="quiet" onclick={showLibrary}>Library</button>
            </div>
          {:else if sessionCount > 0}
            <div class="session-done">
              <h2>Session complete</h2>
              <p>
                {sessionCount}
                {sessionCount === 1 ? "review" : "reviews"} saved.
              </p>
              <button class="primary" onclick={showLibrary}>Library</button>
              <button class="quiet" onclick={() => startPractice(true)}
                >Practise again</button
              >
            </div>
          {:else}
            <div class="session-done">
              <h2>
                {practiceTotal ? "No practice prompts yet." : "No facts yet."}
              </h2>

              {#if practiceTotal}
                <p>
                  Generate prompts with <code
                    >keepsake generate ID --agent claude</code
                  >.
                </p>
              {:else}
                <button
                  class="primary"
                  onclick={() => {
                    view = "library";
                    showAdd = true;
                  }}
                >
                  Add a fact
                </button>
              {/if}
            </div>
          {/if}
        </section>
        {#if selectedMode}
          <details class="study-notes">
            <summary>Practice notes</summary>
            <p>
              {selectedMode.instruction}
              {#if practiceMode === "scheduled"}
                Only an unaided recall attempt changes the review schedule.
                Viewing an answer delays assessment for ten minutes.
              {:else}
                Extra practice samples up to 20 prompts. Recognition,
                explanation and application do not advance the recall schedule.
              {/if}
            </p>
          </details>
        {/if}
      </div>
    {:else}
      <h1 class="sr-only">Match pairs</h1>

      {#if matchingLoading}
        <p role="status">Loading matching pairs…</p>
      {:else if matchingError}
        <div class="message error" role="alert">{matchingError}</div>
        <button class="quiet" onclick={startMatching}>Try again</button>
      {:else if matchFacts.length < 3}
        <section class="empty-state">
          <h2>Add three related facts with a shared topic tag to play.</h2>
          <button class="primary" onclick={() => startPractice()}>
            Practise
          </button>
        </section>
      {:else if matchingBlocked}
        <!-- The global save status or retry alert explains why the round is hidden. -->
      {:else}
        <section class="matching-stage" aria-label="Matching pairs game">
          <p class="card-meta">
            {matched.length} of {matchFacts.length} matched
          </p>
          <div class="matching-columns">
            <div>
              <h2>Questions</h2>

              {#each matchFacts as fact}
                <button
                  class="match-tile"
                  class:selected={selectedPrompt === fact.id}
                  class:matched={matched.includes(fact.id)}
                  disabled={matched.includes(fact.id) ||
                    pendingMatch !== null ||
                    recording ||
                    matchingBlocked}
                  onclick={() => choosePrompt(fact.id)}
                >
                  {#if matched.includes(fact.id)}
                    <span aria-label="Matched">✓</span>
                  {/if}
                  {fact.prompt}
                </button>
              {/each}
            </div>
            <div>
              <h2>Answers</h2>

              {#each matchAnswers as fact}
                <button
                  class="match-tile"
                  class:wrong={selectedAnswer === fact.id &&
                    selectedAnswer !== selectedPrompt}
                  class:selected={selectedAnswer === fact.id &&
                    selectedAnswer === selectedPrompt}
                  class:matched={matched.includes(fact.id)}
                  disabled={matched.includes(fact.id) ||
                    selectedPrompt === null ||
                    pendingMatch !== null ||
                    recording ||
                    matchingBlocked}
                  onclick={() => chooseAnswer(fact.id)}
                >
                  {#if matched.includes(fact.id)}
                    <span aria-label="Matched">✓</span>
                  {/if}
                  {fact.answer}
                </button>
              {/each}
            </div>
          </div>
          <p class="match-feedback" role="status">
            {matchFeedback || "Choose a question, then its answer."}
          </p>

          {#if pendingMatch !== null}
            <div class="grade-panel">
              <h3>Rate your recognition</h3>

              {#if pendingFact?.context}
                <p class="context-copy">{pendingFact.context}</p>
              {/if}

              {#if pendingFact?.source}
                <p class="source">
                  Source:
                  {#if sourceUrl(pendingFact.source)}
                    <a
                      href={pendingFact.source}
                      title={pendingFact.source}
                      target="_blank"
                      rel="noreferrer"
                    >
                      {pendingFact.source}
                    </a>
                  {:else}
                    {pendingFact.source}
                  {/if}
                </p>
              {/if}

              {#if matchMisses[pendingMatch]}
                <p>
                  {matchMisses[pendingMatch]} incorrect {matchMisses[
                    pendingMatch
                  ] === 1
                    ? "choice"
                    : "choices"}. Choose Again if you guessed.
                </p>
              {:else}
                <p>Choose Again if you guessed.</p>
              {/if}

              <div class="ratings">
                {#each ratings as rating}
                  <button
                    class="rating"
                    disabled={recording}
                    onclick={() => gradeMatch(rating.id)}
                  >
                    <strong>{rating.name}</strong>
                    <span>{rating.detail}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          {#if reviewError}
            <p class="error inline-error" role="alert">{reviewError}</p>
          {/if}

          {#if recording}
            <p role="status">Saving review…</p>
          {/if}

          {#if matched.length === matchFacts.length}
            <div class="form-actions">
              <button class="primary" onclick={startMatching}>Play again</button
              >
              <button class="quiet" onclick={() => startPractice()}>
                Practise
              </button>
            </div>
          {/if}
        </section>
        <p class="study-notes">
          Matching tests recognition, which differs from unaided recall.
        </p>
      {/if}
    {/if}
  </main>
</div>

<style>
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
  .share-selection {
    display: flex;
    gap: 1rem;
    align-items: center;
    margin-bottom: 1rem;
    flex-wrap: wrap;
  }
  .fact-row {
    position: relative;
    padding-left: 2rem !important;
  }
  .fact-select {
    position: absolute;
    left: 0;
    top: 1.4rem;
    width: auto;
    accent-color: #244c44;
  }

  /* Global defaults and accessible focus */
  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    background: #f5f8f7;
    color: #18362f;
    font-family: "Avenir Next", Avenir, "Segoe UI", sans-serif;
    font-size: 16px;
  }

  :global(button),
  :global(input),
  :global(textarea) {
    font: inherit;
  }

  :global(button) {
    cursor: pointer;
    border: 0;
    border-radius: 6px;
  }

  :global(button:disabled) {
    cursor: default;
    opacity: 0.6;
  }

  :global(button:focus-visible),
  :global(a:focus-visible),
  :global(summary:focus-visible),
  :global(input:focus-visible),
  :global(textarea:focus-visible) {
    outline: 3px solid #97712d;
    outline-offset: 4px;
  }

  :global(a) {
    color: #365c6a;
    text-underline-offset: 3px;
  }

  :global(p) {
    line-height: 1.65;
  }

  :global(h1),
  :global(h2),
  :global(h3),
  :global(p) {
    margin-top: 0;
  }

  :global(input),
  :global(textarea) {
    width: 100%;
    background: #fff;
    color: #18362f;
    border: 1px solid #b4c8c0;
    border-radius: 6px;
    padding: 0.8rem 0.9rem;
  }

  :global(textarea) {
    resize: vertical;
    line-height: 1.6;
  }

  :global(input::placeholder),
  :global(textarea::placeholder) {
    color: #65776f;
  }

  /* Application shell and navigation */
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

  .quiet {
    background: transparent;
    color: #244c44;
    padding: 0.8rem 1rem;
    border: 1px solid #c0d0c7;
  }

  .quiet:hover {
    background: #e8f0eb;
  }

  /* Page headings and library */
  main {
    min-height: calc(100vh - 112px);
    padding: 2rem 0 4rem;
  }

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

  h2 {
    font-size: 1.25rem;
    font-weight: 600;
    line-height: 1.4;
  }

  h3 {
    font-size: 1rem;
    font-weight: 600;
  }

  .muted {
    color: #64786c;
    font-size: 0.85rem;
  }

  .library-tools {
    margin-bottom: 0.75rem;
  }

  .search {
    display: block;
    max-width: 460px;
  }

  .search input {
    background: transparent;
  }

  .fact-row {
    padding: 1.15rem 0;
    border-bottom: 1px solid #d4e0d8;
  }

  details {
    margin-bottom: 0.6rem;
  }

  summary {
    cursor: pointer;
    font-size: 0.9rem;
    color: #365c6a;
    line-height: 1.6;
  }

  details[open] > summary {
    margin-bottom: 0.8rem;
  }

  .fact-details {
    margin: 0;
  }

  .fact-details > summary {
    color: #18362f;
  }

  .fact-details h2 {
    display: inline;
    font-size: 1.05rem;
    overflow-wrap: anywhere;
  }

  .fact-body {
    max-width: 53rem;
    padding-left: 1rem;
  }

  .answer-copy,
  .context-copy {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .context-copy {
    color: #52685d;
  }

  .verification-note {
    color: #64786c;
    font-size: 0.82rem;
    margin-top: 1rem;
  }

  .source,
  .tags {
    font-size: 0.82rem;
    color: #64786c;
    overflow-wrap: anywhere;
    margin: 0.8rem 0 0;
  }

  .no-results {
    padding: 2rem 0;
    color: #52685d;
  }

  .empty-state {
    max-width: 45rem;
    margin: 2rem 0;
  }

  code {
    background: #e3ede7;
    padding: 0.2rem 0.35rem;
    border-radius: 4px;
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }

  /* Fact form and messages */
  .add-panel {
    background: #fff;
    border: 1px solid #c6d7cd;
    border-radius: 10px;
    margin: 0 0 2rem;
    padding: 2rem;
  }

  form,
  .optional-fields {
    display: grid;
    gap: 1.1rem;
  }

  form label,
  .response-label {
    display: block;
    font-size: 0.9rem;
    font-weight: 600;
  }

  form input,
  form textarea,
  .response-label textarea {
    display: block;
    margin-top: 0.5rem;
    font-weight: 400;
  }

  .optional {
    font-size: 0.8rem;
    color: #64786c;
    font-weight: 400;
    margin-left: 0.4rem;
  }

  .form-pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.2rem;
  }

  .form-actions {
    display: flex;
    gap: 0.7rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .message {
    padding: 1rem 1.2rem;
    border-radius: 6px;
    margin-bottom: 1.5rem;
  }

  .error {
    background: #f9e9e3;
    color: #833d2b;
  }

  .success {
    background: #e3ede7;
    color: #244c44;
  }

  .text-button {
    background: transparent;
    color: inherit;
    text-decoration: underline;
    padding: 0.2rem 0.5rem;
  }

  .pagination {
    display: flex;
    justify-content: flex-start;
    align-items: center;
    gap: 1rem;
    margin: 1.5rem 0 0;
  }

  /* Practice session */
  .practice-layout {
    display: grid;
    gap: 1rem;
    max-width: 53rem;
  }

  .practice-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .recall-confirmation {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 1.4rem;
    font-size: 0.9rem;
  }

  .recall-confirmation input {
    width: auto;
    accent-color: #244c44;
  }

  .study-notes {
    max-width: 53rem;
    font-size: 0.82rem;
    color: #64786c;
    margin-top: 1rem;
  }

  .study-notes p {
    margin-top: 0.75rem;
  }

  .practice-stage,
  .matching-stage {
    background: #fff;
    border: 1px solid #ccdad2;
    border-radius: 12px;
    padding: 2.2rem;
  }

  .card-meta {
    font-size: 0.85rem;
    color: #52685d;
    margin-bottom: 1.4rem;
  }

  .question {
    font-family: "Iowan Old Style", Palatino, serif;
    font-size: clamp(1.6rem, 3vw, 2.1rem);
    line-height: 1.4;
    font-weight: 500;
    margin-bottom: 1.5rem;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .response-label {
    margin-bottom: 1.4rem;
  }

  .answer-panel {
    margin-top: 2rem;
    padding: 1.5rem;
    background: #f0f5f1;
    border-radius: 6px;
    border-left: 3px solid #618573;
  }

  .answer-panel p:last-child {
    margin-bottom: 0;
  }

  .grade-panel {
    margin-top: 1.8rem;
    padding-top: 1.5rem;
    border-top: 1px solid #d4e0d8;
  }

  .grade-panel > p {
    font-size: 0.85rem;
    color: #64786c;
  }

  .ratings {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.5rem;
  }

  .rating {
    padding: 0.9rem 0.5rem;
    color: #244c44;
    background: #fff;
    border: 1px solid #b4c8c0;
  }

  .rating:hover {
    background: #e8f0eb;
    border-color: #618573;
  }

  .rating strong,
  .rating span {
    display: block;
  }

  .rating span {
    color: #64786c;
    font-size: 0.7rem;
    margin-top: 0.4rem;
    line-height: 1.35;
  }

  .inline-error {
    padding: 1rem;
    border-radius: 6px;
  }

  .session-done p {
    color: #52685d;
  }

  .session-done .quiet {
    margin: 0.8rem 0.5rem 0;
  }

  .choices {
    margin: 0 0 1.5rem;
    padding: 0;
    border: 0;
    display: grid;
    gap: 0.6rem;
  }

  .choices label {
    display: flex;
    align-items: center;
    gap: 1rem;
    border: 1px solid #c6d7cd;
    padding: 1rem;
    border-radius: 6px;
    cursor: pointer;
    line-height: 1.5;
  }

  .choices input {
    width: auto;
    flex-shrink: 0;
    accent-color: #244c44;
  }

  .choices label:has(input:focus-visible) {
    outline: 3px solid #97712d;
    outline-offset: 3px;
  }

  .choices label.chosen {
    border-color: #618573;
    background: #f0f5f1;
  }

  .choices label.correct {
    border-color: #3f7553;
    background: #e3ede7;
  }

  .choices label.incorrect {
    border-color: #ac6b54;
    background: #f9e9e3;
  }

  /* Matching game */
  .matching-columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.5rem;
  }

  .matching-columns h2 {
    font-size: 0.9rem;
    color: #52685d;
  }

  .match-tile {
    display: block;
    width: 100%;
    padding: 1rem;
    margin-bottom: 0.75rem;
    min-height: 95px;
    text-align: left;
    line-height: 1.5;
    color: #18362f;
    background: #f8faf8;
    border: 1px solid #c6d7cd;
    overflow-wrap: anywhere;
  }

  .match-tile.selected {
    background: #dce9e1;
    border-color: #618573;
    box-shadow: inset 0 0 0 1px #618573;
  }

  .match-tile.wrong {
    background: #f9e9e3;
    border-color: #ac6b54;
  }

  .match-tile.matched {
    background: #e3ede7;
    border-color: #a0bdac;
    opacity: 0.65;
  }

  .match-feedback {
    margin: 1.2rem 0 0;
    font-size: 0.9rem;
  }

  /* Accessibility and responsive layouts */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
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
    .mobile-add {
      display: block;
      margin-left: auto;
      font-size: 0.85rem;
      padding: 0.75rem 0.8rem;
    }

    main {
      padding-top: 1.5rem;
    }

    .fact-details h2 {
      font-size: 1rem;
    }

    .form-pair {
      grid-template-columns: 1fr;
    }

    .add-panel,
    .practice-stage,
    .matching-stage {
      padding: 1.3rem;
    }

    .ratings {
      grid-template-columns: 1fr 1fr;
    }

    .rating {
      padding: 0.9rem 0.6rem;
    }

    .rating span {
      font-size: 0.75rem;
    }

    .optional {
      display: block;
      margin: 0.3rem 0 0;
    }

    .matching-columns {
      gap: 0.65rem;
    }

    .match-tile {
      padding: 0.75rem;
      font-size: 0.85rem;
      min-height: 110px;
    }
  }
</style>
