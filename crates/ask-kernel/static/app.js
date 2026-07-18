/* ASK viewer — boot: mount panels into the overlay shell, install input,
 * connect. The map is the page; panels float above it and collapse to
 * chips. Panels own their DOM; cross-talk via bus.js only. */

import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";
import { mountDispatch } from "./panels/dispatch.js";
import { mountInspect } from "./panels/inspect.js";
import { mountLogview } from "./panels/logview.js";
import { mountMapview } from "./mapview.js";
import { installInputHandlers } from "./input.js";
import { S } from "./state.js";
import { on, emit } from "./bus.js";
import { connect, refreshTracked } from "./net.js";

mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("mount-track"));
mountMapview(document.getElementById("map"));
mountDispatch(document.getElementById("tab-dispatch"));
mountInspect(document.getElementById("tab-inspect"));
mountLogview(document.getElementById("log"), document.getElementById("help"));

// floating panels: collapse to chips (persisted per panel)
const FLOAT_KEY = "ask-floats-v1";
const floatState = (() => {
  try {
    return JSON.parse(localStorage.getItem(FLOAT_KEY) || "{}");
  } catch (_) {
    return {};
  }
})();
const LABELS = { "dock-track": "TRACK", "dock-command": "CMD", "dock-log": "LOG" };
for (const btn of document.querySelectorAll(".float-collapse")) {
  const panel = document.getElementById(btn.dataset.panel);
  const label = LABELS[btn.dataset.panel] || "PANEL";
  const apply = (collapsed) => {
    panel.classList.toggle("collapsed", collapsed);
    btn.textContent = collapsed ? `[${label} +]` : `[${label} −]`;
    floatState[btn.dataset.panel] = collapsed;
    localStorage.setItem(FLOAT_KEY, JSON.stringify(floatState));
    if (S.lastSnap) emit("snapshot", S.lastSnap); // map visible area changed
  };
  btn.addEventListener("click", () => apply(!panel.classList.contains("collapsed")));
  apply(!!floatState[btn.dataset.panel]);
}

// command tabs: DISPATCH | INSPECT
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
