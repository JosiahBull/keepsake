<script lang="ts">
  import type { components } from "@keepsake/client";

  type Media = components["schemas"]["QuestionMedia"];

  let {
    profileId,
    media,
    only,
    compact = false,
  }: {
    profileId: number;
    media: Media[];
    only?: Media["kind"];
    compact?: boolean;
  } = $props();

  const shown = $derived(
    only ? media.filter((item) => item.kind === only) : media,
  );
  const source = (item: Media) =>
    `/api/profiles/${profileId}/media/${item.media_id}`;
</script>

{#if shown.length}
  <div class="question-media" class:compact>
    {#each shown as item (item.media_id)}
      <!-- Spans, not a figure: matching shows images inside a button. -->
      <span class="item">
        {#if item.kind === "image"}
          <img
            src={source(item)}
            alt={item.alt || "Question image"}
            width={item.width}
            height={item.height}
            loading="lazy"
            decoding="async"
          />
        {:else}
          <audio
            controls
            preload="metadata"
            src={source(item)}
            aria-label={item.alt || "Question audio"}
          ></audio>
        {/if}
        {#if item.ai_generated}
          <span class="generated"
            >AI-generated {item.kind === "image" ? "image" : "clip"}</span
          >
        {/if}
      </span>
    {/each}
  </div>
{/if}

<style>
  .question-media {
    display: grid;
    gap: 0.75rem;
    justify-items: start;
    margin: 0 0 1.5rem;
  }

  /* Full width, so a player sized as a share of it does not collapse. */
  .item {
    display: grid;
    gap: 0.35rem;
    justify-items: start;
    width: 100%;
  }

  img {
    display: block;
    max-width: 100%;
    max-height: min(55vh, 28rem);
    width: auto;
    height: auto;
    border-radius: 8px;
    border: 1px solid #c6d7cd;
    background: #fff;
  }

  audio {
    width: min(100%, 26rem);
  }

  .generated {
    font-size: 0.78rem;
    font-weight: 600;
    color: #6b4a00;
    background: #fff4d6;
    border: 1px solid #e9cf8a;
    border-radius: 999px;
    padding: 0.1rem 0.55rem;
  }

  .compact {
    margin: 0.6rem 0 0;
  }

  .compact img {
    max-height: 6rem;
  }

  .compact .generated {
    font-size: 0.7rem;
  }
</style>
