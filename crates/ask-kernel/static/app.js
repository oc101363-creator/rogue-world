/* ASK viewer — entry point: wire buttons, install handlers, connect.
 * Modules: state.js (shared state) · render.js (drawing) · net.js (server)
 * · input.js (keyboard/mouse). This file only assembles them. */

import { el } from "./state.js";
import { hideInspectPopup } from "./render.js";
import { connect, refreshTracked } from "./net.js";
import { installInputHandlers } from "./input.js";
import { mountMapview } from "./mapview.js";
import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";
import { mountDispatch } from "./panels/dispatch.js";

// inspect popup
if (el.inspectClose) {
  el.inspectClose.addEventListener("click", (e) => {
    e.stopPropagation();
    hideInspectPopup();
  });
}

// boot
mountMapview(el.map);
mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
mountDispatch(document.getElementById("tab-dispatch"));
refreshTracked();
installInputHandlers();
connect();
