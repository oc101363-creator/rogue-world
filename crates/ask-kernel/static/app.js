/* ASK viewer — entry point: mount panels, wire tabs, install handlers,
 * connect. Modules: state.js (shared state) · net.js (server) · input.js
 * (keyboard/mouse) · mapview.js + panels/* (UI). This file only assembles. */

import { connect, refreshTracked } from "./net.js";
import { installInputHandlers } from "./input.js";
import { mountMapview } from "./mapview.js";
import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";
import { mountDispatch } from "./panels/dispatch.js";
import { mountInspect } from "./panels/inspect.js";
import { mountLogview } from "./panels/logview.js";
import { on } from "./bus.js";

// boot
mountMapview(document.getElementById("map"));
mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
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
document.addEventListener("ask-activate-inspect", () => activate("inspect"));
on("activate-tab", activate);

refreshTracked();
installInputHandlers();
connect();
