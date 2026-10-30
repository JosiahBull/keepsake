<script lang="ts">
  /** Announced to screen readers and matched by tests, but not shown. */
  let { label }: { label: string } = $props();
</script>

<div class="loading">
  <span class="label" role="status">{label}</span>
  <div class="mark" aria-hidden="true">
    <div class="deck"></div>
    <div class="lift">
      <div class="card">
        <div class="face front"><span></span><span></span></div>
        <div class="face back"><span></span></div>
      </div>
    </div>
  </div>
</div>

<style>
  /* Covers the viewport without catching taps, so the mark sits at the centre of
     the screen wherever the component is placed, and never moves the layout. */
  .loading {
    position: fixed;
    inset: 0;
    z-index: 4;
    display: grid;
    place-items: center;
    padding: env(safe-area-inset-top) 0 env(safe-area-inset-bottom);
    pointer-events: none;
  }

  .label {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

  /* Hidden at first, so a quick load never flashes it. */
  .mark {
    --delay: 250ms;
    --cycle: 2.4s;
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    opacity: 0;
    animation: loading-appear 240ms ease-out var(--delay) forwards;
  }

  .mark > * {
    grid-area: 1 / 1;
  }

  .deck,
  .card {
    width: 44px;
    height: 32px;
  }

  .deck {
    background: #e3ede7;
    border: 1px solid #b4c8c0;
    border-radius: 5px;
    transform: translateY(4px) scale(0.9);
  }

  .lift {
    animation: loading-lift var(--cycle) var(--delay) infinite;
  }

  /* The card turns as one flat layer, and its back face covers the front while
     the back is towards the viewer. WebKit drew the back face over the front
     with preserve-3d and backface-visibility, so neither is used. */
  .card {
    position: relative;
    animation: loading-turn var(--cycle) var(--delay) infinite;
  }

  .face {
    position: absolute;
    inset: 0;
    border: 1px solid #b4c8c0;
    border-radius: 5px;
    box-shadow: 0 1px 3px #18362f14;
  }

  .front {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    background: white;
  }

  .front span {
    width: 20px;
    height: 3px;
    border-radius: 2px;
    background: #244c44;
  }

  .front span + span {
    width: 13px;
    background: #b4c8c0;
  }

  .back {
    display: grid;
    place-items: center;
    background: #e3ede7;
    opacity: 0;
    /* Seen through the turned card, so mirror it back. */
    transform: scaleX(-1);
    animation: loading-reveal var(--cycle) var(--delay) infinite step-end;
  }

  /* The diamond from the Keepsake mark. */
  .back span {
    width: 8px;
    height: 8px;
    border-radius: 1.5px;
    background: #244c44;
    transform: rotate(45deg);
  }

  @keyframes loading-appear {
    to {
      opacity: 1;
    }
  }

  /* Rest, turn over, rest, turn over: 20%, 30%, 20% and 30% of the cycle. */
  @keyframes loading-turn {
    0%,
    20% {
      transform: perspective(200px) rotateY(0deg);
      animation-timing-function: cubic-bezier(0.65, 0, 0.35, 1);
    }
    50%,
    70% {
      transform: perspective(200px) rotateY(180deg);
      animation-timing-function: cubic-bezier(0.65, 0, 0.35, 1);
    }
    100% {
      transform: perspective(200px) rotateY(360deg);
    }
  }

  /* Shows the back face from the middle of the first turn, when the card is
     edge-on, to the middle of the second. */
  @keyframes loading-reveal {
    0% {
      opacity: 0;
    }
    35% {
      opacity: 1;
    }
    85%,
    100% {
      opacity: 0;
    }
  }

  /* The card rises a little while it turns, as if lifted from the deck. */
  @keyframes loading-lift {
    0%,
    20%,
    50%,
    70%,
    100% {
      transform: none;
      animation-timing-function: ease-in-out;
    }
    35%,
    85% {
      transform: scale(1.08);
      animation-timing-function: ease-in-out;
    }
  }

  @keyframes loading-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  /* No movement: the card rests on the deck and gently fades in and out. */
  @media (prefers-reduced-motion: reduce) {
    .lift,
    .card,
    .back {
      animation: none;
    }

    .mark {
      animation:
        loading-appear 240ms ease-out var(--delay) forwards,
        loading-pulse 2s ease-in-out calc(var(--delay) + 240ms) infinite;
    }
  }
</style>
