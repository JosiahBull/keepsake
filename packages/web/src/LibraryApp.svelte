<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { type components, type paths } from "@keepsake/client";
  import createClient from "openapi-fetch";
  import type { Snippet } from "svelte";
  import AppShell from "./AppShell.svelte";
  import LibraryHeading from "./LibraryHeading.svelte";
  import QuestionMedia from "./QuestionMedia.svelte";
  import { preloaded } from "./preload";
  import type { FactDraft, Profile, Screen } from "./profiles";
  import { refreshBadge } from "./pwa";
  import { Rejected, SaveQueue } from "./saves.svelte";
  import { play } from "./sounds";

  let {
    profile,
    initialView = "library",
    onNavigate,
    profileMenu,
    draft = null,
    onDraftOpened,
    onShare,
    blocked = $bindable(false),
    dirty = $bindable(false),
  }: {
    profile: Profile;
    initialView?: "library" | "practice" | "matching";
    onNavigate: (view: Screen) => void;
    profileMenu: Snippet;
    draft?: FactDraft | null;
    onDraftOpened?: () => void;
    onShare: (ids: number[]) => void;
    blocked?: boolean;
    dirty?: boolean;
  } = $props();
  const profileId = untrack(() => profile.id);
  const api = createClient<paths>({
    baseUrl: "/",
    fetch: (request) => preloaded(request) ?? fetch(request),
  });
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
  type FactSample = components["schemas"]["FactSample"];
  type NewReview = components["schemas"]["NewReview"];
  type Question = components["schemas"]["PracticeQuestion"];
  type PracticeSession = components["schemas"]["PracticeSession"];
  type StudyQuestion = components["schemas"]["StudyQuestion"];
  type StudyQueue = components["schemas"]["StudyQueue"];
  type StudySubmission = components["schemas"]["StudySubmission"];
  type StudyResult = components["schemas"]["StudyResult"];
  type ExposureSubmission = components["schemas"]["ExposureSubmission"];
  type MediaUpload = components["schemas"]["MediaUpload"];
  type Media = components["schemas"]["QuestionMedia"];

  /** A file chosen for a new question. It is uploaded when the fact is saved. */
  type Attachment = {
    key: number;
    name: string;
    kind: Media["kind"];
    type: string;
    file: Blob;
    preview: string;
    alt: string;
    upload?: MediaUpload;
  };

  // The server decodes every upload and refuses anything else; these mirror its limits
  // so a file it would refuse is caught before it is sent.
  const MAX_ATTACHMENTS = 4;
  const MAX_MEDIA_BYTES = 5 * 1024 * 1024;
  const MEDIA_TYPES: Record<string, string> = {
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    png: "image/png",
    webp: "image/webp",
    mp3: "audio/mpeg",
    aac: "audio/aac",
    m4a: "audio/mp4",
    wav: "audio/wav",
    ogg: "audio/ogg",
    oga: "audio/ogg",
    opus: "audio/ogg",
    webm: "audio/webm",
    flac: "audio/flac",
  };
  const SUPPORTED_MEDIA =
    "Use a JPEG, PNG or WebP image, or an MP3, M4A, AAC, WAV, Ogg, WebM or FLAC clip.";
  // The server serves at most 2048 px and refuses uploads over 4096 px; phone photos are
  // larger than both.
  const IMAGE_EDGE = 2048;

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

  // Fact form, optionally opened with shared context from the system share sheet
  const initialDraft = untrack(() => draft);
  let showAdd = $state(initialDraft !== null);
  let saving = $state(false);
  let prompt = $state("");
  let answer = $state("");
  let context = $state(initialDraft?.context ?? "");
  let source = $state(initialDraft?.source ?? "");
  let tags = $state("");
  let attachments = $state<Attachment[]>([]);
  let attachmentKey = 0;

  // Saves reach the server in order. The learner waits only for a save that must
  // precede what is shown next, such as the exposure before an answer.
  const saves = new SaveQueue();
  let waiting = $state(0);
  let saveWarning = $state("");

  // Practice session
  let queue = $state<(Question | StudyQuestion)[]>([]);
  let practiceMode = $state<"scheduled" | "extra">("scheduled");
  let studyQueue = $state<StudyQueue | null>(null);
  let checkingNext = $state(false);
  let nextPractice: Promise<PracticeSession | null> | null = null;
  let recalled = $state(false);
  let feedback: { body: StudySubmission; result: StudyResult } | null = null;
  let studyConflict = $state(false);
  let exposureOnly = $state(false);
  const locallyExposed = new Map<number, number>();
  let practiceLoading = $state(false);
  let practiceError = $state("");
  let practiceTotal = $state(0);
  let cursor = $state(0);
  // Identifies the question on screen, so a late save cannot change the next one.
  let shown = 0;
  let revealed = $state(false);
  let revealRequested = $state(false);
  let choicesReady = $state(false);
  let disclosedFacts = $state<number[]>([]);
  const disclosureRequests = new Map<number, string>();
  let response = $state("");
  let choice = $state<string | null>(null);
  let startedAt = $state(0);
  let responseMs = $state(0);
  let sessionCount = $state(0);
  let savedCount = $state(0);
  let reviewError = $state("");

  // Matching round
  let matchFacts = $state<Fact[]>([]);
  let matchingLoading = $state(false);
  let matchingError = $state("");
  let matchAnswers = $state<Fact[]>([]);
  let roundReady = $state(false);
  let nextSample: Promise<FactSample | null> | null = null;
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
  const navigationBlocked = $derived(
    waiting > 0 || saves.failure !== null || saving,
  );

  $effect(() => {
    // Leaving would discard saves that have not reached the server.
    blocked = saves.size > 0 || saving;
    dirty = Boolean(
      response.trim() ||
      prompt.trim() ||
      answer.trim() ||
      context.trim() ||
      source.trim() ||
      tags.trim() ||
      attachments.length,
    );
  });

  $effect(() => {
    // A background save can fail after the learner has moved on, out of view.
    if (saves.failure) play("alert");
  });

  onMount(() => {
    if (initialDraft) onDraftOpened?.();
    if (initialView === "practice") void startPractice();
    else if (initialView === "matching") void startMatching();
    else void loadFacts();
    return () => {
      for (const attachment of attachments)
        URL.revokeObjectURL(attachment.preview);
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

  function mediaType(file: File): string | undefined {
    const declared = file.type.split(";")[0].trim().toLowerCase();
    if (Object.values(MEDIA_TYPES).includes(declared)) return declared;
    if (declared === "audio/x-m4a") return "audio/mp4";
    if (declared === "audio/x-wav" || declared === "audio/wave") return "audio/wav";
    return MEDIA_TYPES[file.name.split(".").pop()?.toLowerCase() ?? ""];
  }

  /**
   * Scale a large photo down and re-encode it as JPEG before uploading. The server turns
   * it into AVIF, so this JPEG is a high-quality intermediate.
   */
  async function prepareImage(file: File, type: string): Promise<[Blob, string]> {
    let bitmap: ImageBitmap;
    try {
      bitmap = await createImageBitmap(file);
    } catch {
      // The server explains a file the browser cannot read.
      return [file, type];
    }
    const scale = Math.min(1, IMAGE_EDGE / Math.max(bitmap.width, bitmap.height));
    if (scale === 1 && file.size <= 1_500_000) {
      bitmap.close();
      return [file, type];
    }
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    const context = canvas.getContext("2d");
    if (!context) {
      bitmap.close();
      return [file, type];
    }
    // JPEG has no transparency.
    context.fillStyle = "#fff";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    bitmap.close();
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", 0.92),
    );
    return blob ? [blob, "image/jpeg"] : [file, type];
  }

  async function chooseMedia(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = [...(input.files ?? [])];
    input.value = "";
    error = "";
    for (const file of files) {
      if (attachments.length >= MAX_ATTACHMENTS) {
        error = `A question can have up to ${MAX_ATTACHMENTS} images or clips.`;
        break;
      }
      const declared = mediaType(file);
      if (!declared) {
        error = `${file.name} is not a supported file. ${SUPPORTED_MEDIA}`;
        continue;
      }
      const kind = declared.startsWith("image/") ? "image" : "audio";
      const [blob, type] =
        kind === "image" ? await prepareImage(file, declared) : [file, declared];
      if (blob.size > MAX_MEDIA_BYTES) {
        error = `${file.name} is larger than 5 MB.`;
        continue;
      }
      attachments.push({
        key: ++attachmentKey,
        name: file.name,
        kind,
        type,
        file: blob,
        preview: URL.createObjectURL(blob),
        alt: "",
      });
    }
  }

  function removeAttachment(key: number) {
    const attachment = attachments.find((item) => item.key === key);
    if (attachment) URL.revokeObjectURL(attachment.preview);
    attachments = attachments.filter((item) => item.key !== key);
  }

  async function uploadAttachment(attachment: Attachment): Promise<MediaUpload> {
    if (attachment.upload) return attachment.upload;
    try {
      const upload = unwrap(
        await api.POST("/api/media", {
          // The server checks the bytes against this declared type.
          headers: { "content-type": attachment.type },
          body: attachment.file as unknown as string,
          bodySerializer: (body: unknown) => body as BodyInit,
        }),
      );
      attachment.upload = upload;
      return upload;
    } catch (caught) {
      throw new Error(`${attachment.name}: ${message(caught)}`);
    }
  }

  function describeMedia(media: Media[]): string {
    const count = (kind: Media["kind"], one: string, many: string) => {
      const total = media.filter((item) => item.kind === kind).length;
      return total ? `${total} ${total === 1 ? one : many}` : "";
    };
    return [
      count("image", "image", "images"),
      count("audio", "audio clip", "audio clips"),
    ]
      .filter(Boolean)
      .join(", ");
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

      const media = [];
      for (const attachment of attachments) {
        const upload = await uploadAttachment(attachment);
        media.push({ media_id: upload.media_id, alt: attachment.alt.trim() });
      }

      unwrap(
        await api.POST("/api/facts", {
          body: {
            media,
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
      for (const attachment of attachments)
        URL.revokeObjectURL(attachment.preview);
      attachments = [];
      prompt = "";
      answer = "";
      context = "";
      source = "";
      tags = "";
      notice = "Fact saved.";
      play("keep");
      search = "";
      clearTimeout(searchTimer);
      void loadFacts(null, []);
    } catch (caught) {
      error = `Could not save the fact. ${message(caught)}`;
      play("alert");
    } finally {
      saving = false;
    }
  }

  // Saves
  /** Saves before showing what it records, such as an answer, so the learner waits for it. */
  async function saveBeforeShowing<T>(
    label: string,
    send: () => Promise<T>,
  ): Promise<T> {
    waiting += 1;
    try {
      return await saves.add(label, send);
    } finally {
      waiting -= 1;
    }
  }

  /** Saves a rating while the learner moves on. A refused rating leaves a warning. */
  function saveReview(send: () => Promise<unknown>): Promise<boolean> {
    return saves.add("Could not save a review.", send).then(
      () => true,
      (caught: unknown) => {
        saveWarning = `A review was not saved. ${message(caught)}`;
        play("alert");
        return false;
      },
    );
  }

  async function postReview(body: NewReview) {
    return unwrapSave(await api.POST("/api/reviews", { body }));
  }

  async function postStudyEvent(body: StudySubmission) {
    return unwrapSave(await api.POST("/api/study/events", { body }));
  }

  /** Resolves true once the exposure is saved, or false if the server refused it. */
  async function recordExposure(
    factIds: number[],
    method: Method | "matching",
  ): Promise<boolean> {
    const body: ExposureSubmission = {
      submission_id: crypto.randomUUID(),
      fact_ids: factIds,
      method,
    };
    factIds.forEach((id) => locallyExposed.set(id, Date.now()));
    try {
      await saveBeforeShowing("Could not save this activity.", async () =>
        unwrapSave(await api.POST("/api/study/exposures", { body })),
      );
      return true;
    } catch (caught) {
      saveWarning = `Could not save this activity. ${message(caught)}`;
      play("alert");
      return false;
    }
  }

  async function discloseFact(event: Event, factId: number) {
    if (!(event.currentTarget as HTMLDetailsElement).open) {
      clearDisclosure(factId);
      return;
    }

    const request = crypto.randomUUID();
    disclosureRequests.set(factId, request);
    if (
      (await recordExposure([factId], "flashcard")) &&
      disclosureRequests.get(factId) === request
    ) {
      disclosedFacts = [...new Set([...disclosedFacts, factId])];
    }
  }

  function clearDisclosure(factId: number) {
    disclosedFacts = disclosedFacts.filter((id) => id !== factId);
    disclosureRequests.delete(factId);
  }

  // Practice actions
  const practiceSample = () =>
    api.GET("/api/practice", { params: { query: { limit: 20 } } });
  const matchingSample = () =>
    api.GET("/api/facts/sample", { params: { query: { limit: 4 } } });

  /** Starts a request early. A failure here falls back to a fresh request when needed. */
  function prefetch<T>(request: Promise<{ data?: T }>): Promise<T | null> {
    return request.then(
      (result) => result.data ?? null,
      () => null,
    );
  }

  function resetQuestion() {
    shown += 1;
    revealed = false;
    revealRequested = false;
    choicesReady = false;
    response = "";
    choice = null;
    startedAt = Date.now();
    responseMs = 0;
    reviewError = "";
    recalled = false;
    feedback = null;
    studyConflict = false;
    exposureOnly = false;
    if (
      practiceMode === "extra" &&
      current?.practice.kind === "multiple_choice"
    ) {
      void exposeChoices(current.fact.id);
    }
  }

  async function exposeChoices(factId: number) {
    const question = shown;
    if (
      (await recordExposure([factId], "multiple_choice")) &&
      question === shown
    ) {
      choicesReady = true;
    }
  }

  async function startPractice(extra = false) {
    if (navigationBlocked) {
      return;
    }

    const request = ++practiceRequest;
    view = "practice";
    onNavigate("practice");
    practiceLoading = true;
    practiceError = "";
    saveWarning = "";
    practiceMode = extra ? "extra" : "scheduled";
    studyQueue = null;
    checkingNext = false;
    queue = [];
    cursor = 0;
    sessionCount = 0;
    savedCount = 0;
    resetQuestion();

    try {
      let session: PracticeSession | StudyQueue;
      if (extra) {
        const prefetched = nextPractice;
        nextPractice = null;
        session = (await prefetched) ?? unwrap(await practiceSample());
      } else {
        // A queue read before earlier saves land could offer their targets again.
        await saves.idle();
        session = unwrap(
          await api.GET("/api/study/queue", {
            params: { query: { limit: 20 } },
          }),
        );
      }

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

  // The queue checked after a session's saves is the next session.
  function continuePractice() {
    if (!studyQueue || navigationBlocked) {
      return;
    }

    practiceRequest += 1;
    queue = studyQueue.items;
    practiceTotal = studyQueue.total_due + studyQueue.new_remaining;
    cursor = 0;
    sessionCount = 0;
    savedCount = 0;
    saveWarning = "";
    resetQuestion();
  }

  async function reveal() {
    if (revealRequested || !current) {
      return;
    }

    responseMs = Math.max(0, Date.now() - startedAt);
    revealRequested = true;
    const question = shown;
    if (scheduledQuestion) {
      exposureOnly = !recalled && !response.trim();
      const body = feedbackSubmission(scheduledQuestion);
      try {
        const result = await saveBeforeShowing(
          "Could not save this activity.",
          () => postStudyEvent(body),
        );
        if (question === shown) {
          feedback = { body, result };
          showAnswer();
        }
      } catch (caught) {
        if (question === shown) {
          studyConflict = true;
          reviewError =
            caught instanceof Rejected && caught.status === 409
              ? "This attempt is no longer current. Refresh practice."
              : `Could not save this activity. ${message(caught)}`;
          play("alert");
        }
      }
      return;
    }

    // Extra practice ignores schedules, so the next session can load during the last rating.
    if (cursor === queue.length - 1) nextPractice ??= prefetch(practiceSample());
    if (
      (await recordExposure([current.fact.id], current.practice.kind)) &&
      question === shown
    ) {
      showAnswer();
    }
  }

  // Only multiple choice has an answer Keepsake can mark, so only it sounds right or wrong.
  function showAnswer() {
    revealed = true;
    play(
      mode !== "multiple_choice"
        ? "flip"
        : choice === current?.practice.answer
          ? "right"
          : "miss",
    );
  }

  function feedbackSubmission(question: StudyQuestion): StudySubmission {
    return {
      submission_id: crypto.randomUUID(),
      target_id: question.target_id,
      practice_id: question.practice.id,
      content_revision: question.content_revision,
      expected_version: question.schedule_version,
      kind: "exposure",
      context: "scheduled",
      method: question.practice.kind,
      outcome: null,
      response_ms: responseMs,
      attempted: recalled || Boolean(response.trim()),
      assisted: false,
      prior_exposure:
        Date.now() - (locallyExposed.get(question.fact.id) ?? -Infinity) <
        600_000,
    };
  }

  function nextQuestion() {
    cursor += 1;
    resetQuestion();
    if (cursor < queue.length) {
      return;
    }
    void completeSession(practiceRequest);
    if (practiceMode === "scheduled") {
      void refreshStudyOverview();
    }
  }

  /** Like the summary's review count, the completion sound waits for every rating to save. */
  async function completeSession(request: number) {
    await saves.idle();
    if (request === practiceRequest && sessionCount > 0) play("done");
  }

  async function refreshStudyOverview() {
    const request = practiceRequest;
    checkingNext = true;
    try {
      // The next queue must reflect every rating from this session.
      await saves.idle();
      const overview = unwrap(
        await api.GET("/api/study/queue", { params: { query: { limit: 20 } } }),
      );
      if (request === practiceRequest) {
        if (overview.budget_reached && !studyQueue?.budget_reached) {
          play("rest");
        }
        studyQueue = overview;
        void refreshBadge(profileId);
      }
    } catch (caught) {
      if (request === practiceRequest) {
        practiceError = `Could not check the next review. ${message(caught)}`;
      }
    } finally {
      if (request === practiceRequest) {
        checkingNext = false;
      }
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

  function grade(outcome: Outcome) {
    if (!current || !revealed || studyConflict) {
      return;
    }

    let saved: Promise<boolean>;
    if (scheduledQuestion) {
      if (!feedback || exposureOnly) {
        return;
      }
      // A rating refers to its own feedback and the schedule state that feedback produced.
      const attempt: StudySubmission = {
        ...feedback.body,
        submission_id: crypto.randomUUID(),
        kind: "attempt",
        outcome,
        expected_version: feedback.result.schedule.version,
        feedback_event_id: feedback.result.event_id,
      };
      saved = saveReview(() => postStudyEvent(attempt));
    } else {
      const review: NewReview = {
        fact_id: current.fact.id,
        practice_id: current.practice.id,
        method: current.practice.kind,
        outcome,
        response_ms: responseMs,
      };
      saved = saveReview(() => postReview(review));
    }

    const session = practiceRequest;
    void saved.then((ok) => {
      if (ok && session === practiceRequest) savedCount += 1;
    });
    play("file");
    sessionCount += 1;
    nextQuestion();
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
    saveWarning = "";
    matchFacts = [];
    matchAnswers = [];
    roundReady = false;
    matched = [];
    selectedPrompt = null;
    selectedAnswer = null;
    pendingMatch = null;
    matchFeedback = "";
    matchStarted = {};
    matchMisses = {};

    try {
      const prefetched = nextSample;
      nextSample = null;
      const sample = (await prefetched) ?? unwrap(await matchingSample());

      if (request !== matchingRequest) {
        return;
      }

      matchFacts = sample.items;
      matchAnswers = shuffled(matchFacts);
      matchingLoading = false;
      // The answers stay hidden until their exposure is saved.
      if (
        matchFacts.length >= 3 &&
        (await recordExposure(
          matchFacts.map((fact) => fact.id),
          "matching",
        )) &&
        request === matchingRequest
      ) {
        roundReady = true;
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
    if (pendingMatch !== null) {
      return;
    }

    selectedPrompt = id;
    selectedAnswer = null;
    matchFeedback = "";
    play("tick");
    if (!matchStarted[id]) {
      matchStarted = { ...matchStarted, [id]: Date.now() };
    }
  }

  function chooseAnswer(id: number) {
    if (selectedPrompt === null || pendingMatch !== null) {
      return;
    }

    selectedAnswer = id;
    if (id === selectedPrompt) {
      pendingMatch = id;
      matchMs = Math.max(0, Date.now() - matchStarted[id]);
      matchFeedback = "Matched. Rate your attempt.";
      play("right");
      // Matching ignores schedules, so the next round can load during the last rating.
      if (matched.length === matchFacts.length - 1) {
        nextSample ??= prefetch(matchingSample());
      }
    } else {
      matchMisses = {
        ...matchMisses,
        [selectedPrompt]: (matchMisses[selectedPrompt] || 0) + 1,
      };
      matchFeedback = "Not a match. Try another answer.";
      play("miss");
    }
  }

  function gradeMatch(outcome: Outcome) {
    if (pendingMatch === null) {
      return;
    }

    const review: NewReview = {
      fact_id: pendingMatch,
      practice_id: null,
      method: "matching",
      outcome,
      response_ms: matchMs,
    };
    void saveReview(() => postReview(review));
    matched = [...matched, pendingMatch];
    pendingMatch = null;
    selectedPrompt = null;
    selectedAnswer = null;
    const complete = matched.length === matchFacts.length;
    matchFeedback = complete ? "Round complete." : "Choose another question.";
    play(complete ? "done" : "file");
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

  /** Like unwrap, but marks a refusal that resending cannot change as Rejected. */
  function unwrapSave<T>(result: {
    data?: T;
    error?: components["schemas"]["ErrorBody"];
    response: Response;
  }): T {
    const { status } = result.response;
    if (
      result.error &&
      status >= 400 &&
      status < 500 &&
      status !== 408 &&
      status !== 429
    ) {
      throw new Rejected(result.error.error, status);
    }

    return unwrap(result);
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

{#snippet savedReviews()}
  {#if saves.size}
    <p role="status">Saving…</p>
  {:else}
    <p>{savedCount} {savedCount === 1 ? "review" : "reviews"} saved.</p>
  {/if}
{/snippet}

<AppShell
  {profileId}
  current={view}
  disabled={navigationBlocked}
  adding={showAdd}
  onNavigate={(next) => {
    if (next === "library") showLibrary();
    else if (next === "practice") void startPractice();
    else void startMatching();
  }}
  onAdd={openAdd}
  {profileMenu}
>
  {#if waiting && view !== "practice"}
    <p role="status">Saving…</p>
  {/if}
  {#if saves.failure}
    <div class="message error" role="alert">
      {saves.failure.label}
      {message(saves.failure.error)}
      <button class="text-button" onclick={() => saves.retry()}
        >Retry save</button
      >
    </div>
  {/if}
  {#if saveWarning}
    <div class="message error" role="alert">{saveWarning}</div>
  {/if}
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
    <LibraryHeading
      current="library"
      disabled={blocked}
      addDisabled={navigationBlocked}
      adding={showAdd}
      onAdd={openAdd}
      {onNavigate}
    >
      {#if !error && !loading}
        <span class="muted">
          {totalFacts}
          {totalFacts === 1 ? "fact" : "facts"}
        </span>
      {/if}
    </LibraryHeading>

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
          <div class="media-field">
            <label>
              Images and audio
              <span class="optional">optional · up to 4 · 5 MB each</span>
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp,audio/*"
                multiple
                disabled={attachments.length >= MAX_ATTACHMENTS || saving}
                onchange={chooseMedia}
              />
            </label>
            {#each attachments as attachment (attachment.key)}
              <div class="attachment">
                {#if attachment.kind === "image"}
                  <img src={attachment.preview} alt="" />
                  <label>
                    Describe the image
                    <span class="optional">for screen readers; do not give away the answer</span>
                    <input bind:value={attachment.alt} maxlength="500" />
                  </label>
                {:else}
                  <audio
                    controls
                    src={attachment.preview}
                    aria-label={attachment.name}
                  ></audio>
                {/if}
                <button
                  type="button"
                  class="text-button"
                  disabled={saving}
                  onclick={() => removeAttachment(attachment.key)}
                  >Remove {attachment.name}</button
                >
              </div>
            {/each}
          </div>
          <details
            class="form-details"
            open={Boolean(initialDraft?.context || initialDraft?.source)}
          >
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
                {#if fact.media.length}<span class="media-note"
                    >{describeMedia(fact.media)}</span
                  >{/if}
              </summary>
              {#if disclosedFacts.includes(fact.id)}
                <div class="fact-body">
                  <QuestionMedia profileId={profileId} media={fact.media} />
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
          <QuestionMedia profileId={profileId} media={current.fact.media} />

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
              disabled={revealRequested ||
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
                  disabled={studyConflict}
                  onclick={() => {
                    play("file");
                    nextQuestion();
                  }}>Continue</button
                >
              {:else}
                <h3>Rate your attempt</h3>
                <div class="ratings">
                  {#each ratings as rating}
                    <button
                      class="rating"
                      disabled={studyConflict}
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
          {#if studyConflict}
            <p class="error inline-error" role="alert">{reviewError}</p>
            <button class="quiet" onclick={() => startPractice()}
              >Refresh practice</button
            >
          {/if}
          {#if waiting}
            <p role="status">Saving…</p>
          {/if}
        {:else if practiceMode === "scheduled"}
          <div class="session-done">
            <h2>
              {sessionCount > 0
                ? "Session complete"
                : "Nothing due right now"}
            </h2>
            {#if sessionCount > 0}{@render savedReviews()}{/if}
            {#if !checkingNext && studyQueue?.next_due_at}<p>
                Next review: {dueLabel(studyQueue.next_due_at)}.
              </p>{/if}
            {#if !checkingNext && studyQueue && studyQueue.items.length > 0}
              <button class="primary" onclick={continuePractice}
                >Continue practice</button
              >
            {/if}
            <button class="quiet" onclick={showLibrary}>Library</button>
          </div>
        {:else if sessionCount > 0}
          <div class="session-done">
            <h2>Session complete</h2>
            {@render savedReviews()}
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
    {:else if !roundReady}
      <!-- The save status or an alert explains why the round is hidden. -->
    {:else}
      <section class="matching-stage" aria-label="Matching pairs game">
        <p class="card-meta">
          {matched.length} of {matchFacts.length} matched
        </p>
        <div class="matching-columns">
          <div>
            <h2>Questions</h2>

            {#each matchFacts as fact}
              <div class="match-item">
                <button
                  class="match-tile"
                  class:selected={selectedPrompt === fact.id}
                  class:matched={matched.includes(fact.id)}
                  disabled={matched.includes(fact.id) || pendingMatch !== null}
                  onclick={() => choosePrompt(fact.id)}
                >
                  {#if matched.includes(fact.id)}
                    <span aria-label="Matched">✓</span>
                  {/if}
                  {fact.prompt}
                  <QuestionMedia
                    profileId={profileId}
                    media={fact.media}
                    only="image"
                    compact
                  />
                </button>
                <!-- A player cannot sit inside a button, so clips follow their tile. -->
                <QuestionMedia
                  profileId={profileId}
                  media={fact.media}
                  only="audio"
                  compact
                />
              </div>
            {/each}
          </div>
          <div>
            <h2>Answers</h2>

            {#each matchAnswers as fact}
              <div class="match-item">
                <button
                  class="match-tile"
                  class:wrong={selectedAnswer === fact.id &&
                    selectedAnswer !== selectedPrompt}
                  class:selected={selectedAnswer === fact.id &&
                    selectedAnswer === selectedPrompt}
                  class:matched={matched.includes(fact.id)}
                  disabled={matched.includes(fact.id) ||
                    selectedPrompt === null ||
                    pendingMatch !== null}
                  onclick={() => chooseAnswer(fact.id)}
                >
                  {#if matched.includes(fact.id)}
                    <span aria-label="Matched">✓</span>
                  {/if}
                  {fact.answer}
                </button>
              </div>
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
                <button class="rating" onclick={() => gradeMatch(rating.id)}>
                  <strong>{rating.name}</strong>
                  <span>{rating.detail}</span>
                </button>
              {/each}
            </div>
          </div>
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
</AppShell>

<style>
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
    /* Quick taps on buttons and tiles must not zoom the page; pinch zoom still works. */
    touch-action: manipulation;
  }

  :global(body) {
    margin: 0;
    /* An installed app with viewport-fit=cover can draw under the notch and home indicator. */
    padding: env(safe-area-inset-top) env(safe-area-inset-right)
      env(safe-area-inset-bottom) env(safe-area-inset-left);
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
    /* iOS zooms into a focused field whose text is smaller than 16px. */
    font-size: 1rem;
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

  /* Library */
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

  .media-field {
    display: grid;
    gap: 0.8rem;
  }

  .media-field input[type="file"] {
    display: block;
    margin-top: 0.4rem;
    padding: 0;
    border: 0;
    background: transparent;
    color: #52685d;
    /* iOS zooms into a focused field whose text is smaller than 16px. */
    font-size: 1rem;
    font-weight: 400;
  }

  .media-field input[type="file"]::file-selector-button {
    font: inherit;
    font-weight: 600;
    color: #244c44;
    background: transparent;
    border: 1px solid #c0d0c7;
    border-radius: 6px;
    padding: 0.6rem 0.9rem;
    margin-right: 0.8rem;
    cursor: pointer;
  }

  .attachment {
    display: grid;
    gap: 0.5rem;
    justify-items: start;
    padding: 0.8rem;
    border: 1px solid #dbe5df;
    border-radius: 8px;
  }

  .attachment img {
    max-height: 8rem;
    max-width: 100%;
    border-radius: 6px;
  }

  .attachment audio {
    width: min(100%, 26rem);
  }

  .attachment label {
    width: 100%;
  }

  .media-note {
    color: #64786c;
    font-size: 0.82rem;
    margin-left: 0.5rem;
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

  .pagination button {
    color: #51675e;
    padding: 0.7rem 0.9rem;
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

  .match-item {
    margin-bottom: 0.75rem;
  }

  .match-tile {
    display: block;
    width: 100%;
    padding: 1rem;
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
    .pagination button {
      padding: 0.65rem;
      font-size: 0.9rem;
    }
  }

  @media (max-width: 580px) {
    .pagination button {
      flex: 1;
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
