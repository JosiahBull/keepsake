import { mount } from "svelte";
import App from "./App.svelte";
import Install from "./Install.svelte";
import { standalone } from "./pwa";

// The site's front page shows how to install Keepsake; the app itself starts at /app.
// Installed apps, and Home Screen shortcuts from before the app moved, still open the app.
if (
  location.pathname === "/" &&
  (standalone() || new URLSearchParams(location.search).has("open"))
)
  history.replaceState(history.state, "", `/app${location.search}`);

mount(location.pathname === "/" ? Install : App, {
  target: document.getElementById("app")!,
});
