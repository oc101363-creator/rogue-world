/* HUD panel: connection status, mode, info line, cam readout, theme
 * select, FOLLOW/MOCK, dock collapse toggles. Owns #hud. */

import { S } from "../state.js";
import { on, emit, log } from "../bus.js";
import { THEMES, getTheme } from "../themes.js";

const DOCK_KEY = "ask-docks-v1";

function loadDocks() {
  try {
    return JSON.parse(localStorage.getItem(DOCK_KEY) || "{}");
  } catch (_) {
    return {};
  }
}

export function mountHud(root) {
  root.innerHTML = `
    <span id="status">…</span>
    <span id="mode" class="mode-mock">MOCK</span>
    <span id="info"></span>
    <span id="cam"></span>
    <span class="spacer"></span>
    <label for="theme">THEME</label>
    <select id="theme" aria-label="Map color theme"></select>
    <button type="button" id="btn-follow" class="term-btn" title="Follow focused agent">[ FOLLOW ]</button>
    <button type="button" id="btn-mock" class="term-btn secondary" title="Toggle mock">[ MOCK ]</button>
    <button type="button" id="dock-toggle-l" class="term-btn secondary" title="Toggle left dock">[ ◀ ]</button>
    <button type="button" id="dock-toggle-r" class="term-btn secondary" title="Toggle right dock">[ ▶ ]</button>`;

  const status = root.querySelector("#status");
  const mode = root.querySelector("#mode");
  const info = root.querySelector("#info");
  const cam = root.querySelector("#cam");
  const themeSel = root.querySelector("#theme");

  // theme select (moved from setupThemeSelect + applyThemeChrome)
  const applyChrome = () => {
    const u = S.theme.ui;
    const rs = document.documentElement.style;
    rs.setProperty("--bg", u.bg);
    rs.setProperty("--hud", u.hud);
    rs.setProperty("--hud-muted", u.hudMuted);
    rs.setProperty("--online", u.online);
    rs.setProperty("--offline", u.offline);
    document.body.style.background = u.bg;
    document.getElementById("viewport").style.background = u.bg;
  };
  for (const t of THEMES) {
    const opt = document.createElement("option");
    opt.value = t.id;
    opt.textContent = t.name;
    themeSel.appendChild(opt);
  }
  themeSel.value = S.theme.id;
  themeSel.addEventListener("change", () => {
    S.theme = getTheme(themeSel.value);
    localStorage.setItem("ask-theme", S.theme.id);
    applyChrome();
    emit("theme-changed");
    if (S.lastSnap) emit("snapshot", S.lastSnap); // force redraw
  });
  applyChrome();

  // dock collapse (persisted)
  const docks = loadDocks();
  document.body.classList.toggle("l-collapsed", !!docks.l);
  document.body.classList.toggle("r-collapsed", !!docks.r);
  const toggle = (side) => {
    const cls = side === "l" ? "l-collapsed" : "r-collapsed";
    document.body.classList.toggle(cls);
    const d = loadDocks();
    d[side] = document.body.classList.contains(cls);
    localStorage.setItem(DOCK_KEY, JSON.stringify(d));
    emit("camera-changed"); // viewport resized → reclamp
    if (S.lastSnap) emit("snapshot", S.lastSnap); // force redraw
  };
  root.querySelector("#dock-toggle-l").addEventListener("click", () => toggle("l"));
  root.querySelector("#dock-toggle-r").addEventListener("click", () => toggle("r"));

  root.querySelector("#btn-follow").addEventListener("click", () => {
    S.cam.follow = true;
    log("FOLLOW ON");
    if (S.lastSnap) emit("snapshot", S.lastSnap);
  });
  root.querySelector("#btn-mock").addEventListener("click", async () => {
    const { setHumanControl } = await import("../net.js");
    setHumanControl(false);
    log("MOCK");
  });

  // bus subscriptions
  on("conn-status", ({ text, online }) => {
    status.textContent = text;
    status.className = online ? "online" : "offline";
  });
  on("hud-info", (text) => { info.textContent = text; });
  on("mode-changed", (human) => {
    mode.textContent = human ? "HUMAN" : "MOCK";
    mode.className = human ? "mode-human" : "mode-mock";
  });
  on("camera-changed", () => {
    cam.textContent = `cam(${S.cam.tx},${S.cam.ty}) z${S.cam.zi}`;
  });
}
