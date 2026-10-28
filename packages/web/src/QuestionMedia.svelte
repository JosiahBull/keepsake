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

  .compact {
    margin: 0.6rem 0 0;
  }

  .compact img {
    max-height: 6rem;
  }
</style>
