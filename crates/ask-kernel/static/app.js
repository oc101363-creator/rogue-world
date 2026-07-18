/* ASK viewer — boot: mount panels into the grid shell, install input,
 * connect. Panels own their DOM; cross-talk via bus.js only. */

import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";
import { mountDispatch } from "./panels/dispatch.js";
import { mountInspect } from "./panels/inspect.js";
import { mountLogview } from "./panels/logview.js";
import { mountMapview } from "./mapview.js";
import { installInputHandlers } from "./input.js";
import { on } from "./bus.js";
import { connect, refreshTracked } from "./net.js";

mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
mountMapview(document.getElementById("map"));
mountDispatch(document.getElementById("tab-dispatch"));
mountInspect(document.getElementById("tab-inspect"));
mountLogview(document.getElementById("log"), document.getElementById("help"));

// right-dock tabs: DISPATCH | INSPECT
const tabs = document.getElementById("command-tabs");
const activate = (name) => {
  for (const b of tabs.querySelectorAll(".tab-btn"))
    b.classList.toggle("active", b.dataset.tab === name);
  document.getElementById("tab-dispatch").classList.toggle("active", name === "dispatch");
  document.getElementById("tab-inspect").classList.toggle("active", name === "inspect");
};
tabs.addEventListener("click", (e) => {
  if (e.target.dataset.tab) activate(e.target.dataset.tab);
});
on("activate-tab", activate);

installInputHandlers();
refreshTracked();
connect();
