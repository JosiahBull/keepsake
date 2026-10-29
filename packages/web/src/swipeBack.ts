// Swiping in from the left edge of the screen goes back, as in iOS apps. Only the
// Home Screen app needs it: in Safari the browser's own edge swipe would also fire.

const EDGE = 24;

/** True in an iOS Home Screen web app, which has no back button or back gesture. */
export function iosHomeScreenApp(): boolean {
  return (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

/**
 * Lets a touch that starts at the left edge drag the page to the right. Released
 * past a third of the screen, or flicked, the page slides away and `back` runs;
 * otherwise it slides home. Returns a function that removes the gesture.
 */
export function edgeSwipeBack(
  canGoBack: () => boolean,
  back: () => void,
): () => void {
  const root = document.documentElement;
  let start: { x: number; y: number; pointer: number; time: number } | null =
    null;
  let page: HTMLElement | null = null;
  let settling = false;

  // A pinch-zoomed page needs sideways drags to pan.
  const zoomed = () => (visualViewport?.scale ?? 1) > 1.01;
  // Without horizontal panning, the browser hands sideways drags to the page.
  const claimDrags = () => root.classList.toggle("edge-swipe", !zoomed());

  const settle = (moved: HTMLElement) => {
    moved.style.transition = "none";
    moved.style.transform = "";
    root.classList.remove("swiping-back");
    requestAnimationFrame(() => {
      moved.style.transition = "";
      settling = false;
    });
  };

  const down = (event: PointerEvent) => {
    if (
      settling ||
      event.pointerType === "mouse" ||
      !event.isPrimary ||
      event.clientX > EDGE ||
      zoomed() ||
      document.querySelector("dialog[open]") ||
      !canGoBack()
    )
      return;
    start = {
      x: event.clientX,
      y: event.clientY,
      pointer: event.pointerId,
      time: event.timeStamp,
    };
    page = null;
  };

  const move = (event: PointerEvent) => {
    if (!start || event.pointerId !== start.pointer) return;
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    if (!page) {
      if (Math.abs(dy) > 10 && Math.abs(dy) > dx) {
        start = null;
        return;
      }
      if (dx < 10) return;
      page = document.querySelector<HTMLElement>(".app-shell main");
      if (!page) {
        start = null;
        return;
      }
      root.classList.add("swiping-back");
      page.style.transition = "none";
    }
    page.style.transform = `translateX(${Math.max(0, dx)}px)`;
  };

  const up = (event: PointerEvent) => {
    if (!start || event.pointerId !== start.pointer) return;
    const moved = page;
    const dx = event.clientX - start.x;
    const speed = dx / Math.max(1, event.timeStamp - start.time);
    start = null;
    page = null;
    if (!moved) return;

    const going =
      event.type === "pointerup" && (dx > innerWidth / 3 || speed > 0.5);
    settling = true;
    moved.style.transition = "transform 0.2s ease-out";
    moved.style.transform = going ? `translateX(${innerWidth}px)` : "";
    setTimeout(() => {
      if (!going) {
        settle(moved);
        return;
      }
      // The previous screen renders on popstate; it then appears in place.
      const arrived = () => {
        clearTimeout(fallback);
        requestAnimationFrame(() => settle(moved));
      };
      const fallback = setTimeout(() => {
        removeEventListener("popstate", arrived);
        settle(moved);
      }, 1000);
      addEventListener("popstate", arrived, { once: true });
      back();
    }, 200);
  };

  claimDrags();
  visualViewport?.addEventListener("resize", claimDrags);
  addEventListener("pointerdown", down);
  addEventListener("pointermove", move);
  addEventListener("pointerup", up);
  addEventListener("pointercancel", up);
  return () => {
    root.classList.remove("edge-swipe", "swiping-back");
    visualViewport?.removeEventListener("resize", claimDrags);
    removeEventListener("pointerdown", down);
    removeEventListener("pointermove", move);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", up);
  };
}
