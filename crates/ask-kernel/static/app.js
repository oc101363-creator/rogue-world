/* ASK map viewer — pan/zoom + player sandbox controls */

const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

const elViewport = document.getElementById("viewport");
const elMap = document.getElementById("map");
const elStatus = document.getElementById("status");
const elInfo = document.getElementById("info");
const elCam = document.getElementById("cam");
const elTheme = document.getElementById("theme");
const elMode = document.getElementById("mode");
const elLog = document.getElementById("log");
const elTokenInput = document.getElementById("token-input");
const elTokenAdd = document.getElementById("token-add");
const elTokenClear = document.getElementById("token-clear");
const elTrackerList = document.getElementById("tracker-list");
const elTrackerHint = document.getElementById("tracker-hint");
const elBtnFollow = document.getElementById("btn-follow");
const elBtnMock = document.getElementById("btn-mock");
const elInspectPopup = document.getElementById("inspect-popup");
const elInspectTitle = document.getElementById("inspect-title");
const elInspectBody = document.getElementById("inspect-body");
const elInspectClose = document.getElementById("inspect-close");
const elSelectBox = document.getElementById("select-box");

const elSelCount = document.getElementById("sel-count");
const elSelPreset = document.getElementById("sel-preset");
const elSelPresetDel = document.getElementById("sel-preset-del");
const elSelPresetSave = document.getElementById("sel-preset-save");
const elSelPresetName = document.getElementById("sel-preset-name");
const elSelText = document.getElementById("sel-text");
const elSelSend = document.getElementById("sel-send");

const ZOOM_STEPS = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40];
const THEME_KEY = "ask-theme";
const TRACK_KEY = "ask-track-tokens";
const PRESETS_KEY = "ask-presets-v1";
const TRACK_COLORS = ["#ffff00", "#00ffff", "#00ff00", "#ff00ff", "#ff8800", "#88ff88"];

let display = null;
let mapW = 0;
let mapH = 0;
let lastSnap = null;
let viewCols = 0;
let viewRows = 0;
let theme = getTheme(localStorage.getItem(THEME_KEY) || "rogue-80");
let ws = null;
let humanControl = false;
let lastMe = null;
/** pending direction for o/c/f/t commands */
let pendingDirCmd = null; // "open" | "close" | "attack" | "dig" | "place" | "scoop" | null
/** @type {{token:string, agent_id?:number, name?:string, purpose?:string, x?:number, y?:number, color:string}[]} */
let tracked = loadTracked();
let followToken = tracked.length ? tracked[0].token : null; // which tracked agent camera follows

let selecting = false;
let selectStart = null; // { sx, sy, wx, wy }
let selectedAgentIds = new Set();

const cam = {
  tx: 0,
  ty: 0,
  zi: 4,
  follow: true,
};

function loadTracked() {
  try {
    const raw = JSON.parse(localStorage.getItem(TRACK_KEY) || "[]");
    if (!Array.isArray(raw)) return [];
    return raw.map((t, i) => ({
      token: String(t.token || t),
      agent_id: t.agent_id,
      name: t.name,
      purpose: t.purpose,
      x: t.x,
      y: t.y,
      color: t.color || TRACK_COLORS[i % TRACK_COLORS.length],
    }));
  } catch (_) {
    return [];
  }
}

function saveTracked() {
  localStorage.setItem(
    TRACK_KEY,
    JSON.stringify(
      tracked.map((t) => ({
        token: t.token,
        agent_id: t.agent_id,
        name: t.name,
        purpose: t.purpose,
        color: t.color,
      })),
    ),
  );
}

function renderTracker() {
  if (!elTrackerList) return;
  elTrackerList.innerHTML = "";
  tracked.forEach((t, i) => {
    const div = document.createElement("div");
    div.className = "track-item" + (followToken === t.token ? " active" : "");
    div.innerHTML =
      `<button type="button" class="rm" data-i="${i}" title="remove">[x]</button>` +
      `<div class="name" style="color:${t.color}">@ ${t.name || "agent"}</div>` +
      `<div class="meta">id=${t.agent_id ?? "?"}  @(${t.x ?? "?"},${t.y ?? "?"})</div>` +
      `<div class="tok">${t.token.slice(0, 18)}…</div>`;
    div.addEventListener("click", (e) => {
      if (e.target.classList.contains("rm")) return;
      followToken = t.token;
      cam.follow = true;
      sendSubscribe();
      if (t.x != null && t.y != null) {
        centerOnTile(t.x, t.y);
        if (lastSnap) drawSnap(lastSnap);
      }
      renderTracker();
      pushLog(`FOLLOW ${t.name || t.token.slice(0, 12)}`);
    });
    div.querySelector(".rm").addEventListener("click", (e) => {
      e.stopPropagation();
      tracked.splice(i, 1);
      if (followToken === t.token) followToken = tracked.length ? tracked[0].token : null;
      saveTracked();
      renderTracker();
      sendSubscribe();
    });
    elTrackerList.appendChild(div);
  });
  if (elTrackerHint) elTrackerHint.textContent = `${tracked.length} tracked (saved locally)`;
}

async function refreshTracked() {
  if (!tracked.length) {
    renderTracker();
    return;
  }
  await Promise.all(
    tracked.map(async (t) => {
      try {
        const r = await fetch("/api/track?token=" + encodeURIComponent(t.token));
        if (!r.ok) return;
        const d = await r.json();
        if (!d.ok) return;
        t.agent_id = d.agent_id;
        t.name = d.name;
        t.purpose = d.purpose;
        t.x = d.x;
        t.y = d.y;
      } catch (_) {}
    }),
  );
  saveTracked();
  renderTracker();
  if (cam.follow && followToken) {
    const t = tracked.find((x) => x.token === followToken);
    if (t && t.x != null) {
      centerOnTile(t.x, t.y);
      if (lastSnap) drawSnap(lastSnap);
    }
  }
}

async function addToken(tok) {
  const token = (tok || "").trim();
  if (!token) return;
  if (tracked.some((t) => t.token === token)) {
    pushLog("already tracked");
    return;
  }
  const color = TRACK_COLORS[tracked.length % TRACK_COLORS.length];
  tracked.push({ token, color });
  saveTracked();
  await refreshTracked();
  followToken = token;
  cam.follow = true;
  sendSubscribe();
  pushLog(`+TRACK ${token.slice(0, 16)}…`);
}

async function refreshMe() {
  const token = followToken || (tracked[0] && tracked[0].token);
  if (!token) return;
  try {
    const r = await fetch("/api/me?token=" + encodeURIComponent(token));
    if (!r.ok) return;
    const d = await r.json();
    if (!d.ok) return;
    lastMe = d;
    const t = tracked.find((x) => x.token === token);
    if (t) {
      t.agent_id = d.id;
      if (d.name) t.name = d.name;
      t.x = d.x;
      t.y = d.y;
    }
    saveTracked();
    renderTracker();
  } catch (_) {}
}

function focusAgent() {
  if (!lastSnap) return null;
  const trackedIds = new Set(
    tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  const agents = (lastSnap.entities || []).filter(
    (e) => e.kind === "agent" && trackedIds.has(e.id),
  );
  if (followToken) {
    const t = tracked.find((x) => x.token === followToken);
    if (t && t.agent_id != null) {
      return agents.find((e) => e.id === t.agent_id) || null;
    }
  }
  return agents[0] || null;
}

function clearTracked() {
  tracked = [];
  followToken = null;
  lastMe = null;
  saveTracked();
  renderTracker();
  pushLog("CLEARED tracked tokens");
  if (lastSnap) {
    drawSnap(lastSnap);
  }
  sendSubscribe();
}

function sendSubscribe() {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  const focus = followToken || (tracked[0] && tracked[0].token) || null;
  ws.send(
    JSON.stringify({
      type: "subscribe",
      tokens: tracked.map((t) => t.token),
      focus,
    }),
  );
}

function cellSize() {
  return ZOOM_STEPS[cam.zi];
}

function worldAtScreen(clientX, clientY) {
  const mapRect = elMap.getBoundingClientRect();
  const cs = cellSize();
  const sx = clientX - mapRect.left;
  const sy = clientY - mapRect.top;
  return {
    wx: Math.floor(cam.tx + sx / cs),
    wy: Math.floor(cam.ty + sy / cs),
    sx,
    sy,
  };
}

function updateSelectionHighlight() {
  // drawSnap already reads selectedAgentIds globally
  if (lastSnap) drawSnap(lastSnap);
}

function loadPresets() {
  try {
    const raw = JSON.parse(localStorage.getItem(PRESETS_KEY) || "[]");
    return Array.isArray(raw) ? raw : [];
  } catch (_) {
    return [];
  }
}

function savePresets(presets) {
  localStorage.setItem(PRESETS_KEY, JSON.stringify(presets));
}

function renderPresets() {
  if (!elSelPreset) return;
  const presets = loadPresets();
  elSelPreset.innerHTML = "";
  const none = document.createElement("option");
  none.value = "";
  none.textContent = "-- preset --";
  elSelPreset.appendChild(none);
  for (const p of presets) {
    const opt = document.createElement("option");
    opt.value = p.id;
    opt.textContent = p.name;
    elSelPreset.appendChild(opt);
  }
}

function updateSelectionPanel() {
  if (!elSelCount) return;
  elSelCount.textContent = `${selectedAgentIds.size} agents selected`;
}

async function sendPromptToSelected(text) {
  const token = inspectToken();
  if (!token) {
    pushLog("SEND: track a token first");
    return;
  }
  if (!selectedAgentIds.size) {
    pushLog("SEND: select agents first");
    return;
  }
  if (!text.trim()) {
    pushLog("SEND: empty prompt");
    return;
  }
  const targets = Array.from(selectedAgentIds);
  try {
    const r = await fetch("/api/message", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token, targets, text }),
    });
    const d = await r.json();
    if (!d.ok) {
      pushLog("SEND: " + (d.reason || "failed"));
      return;
    }
    pushLog(`SEND → ${d.sent} agents, ${d.rejected} rejected`);
  } catch (_) {
    pushLog("SEND: network");
  }
}

function setSelectedAgents(ids) {
  selectedAgentIds = new Set(ids);
  updateSelectionPanel();
  updateSelectionHighlight();
}

function toggleSelectAgent(id) {
  if (selectedAgentIds.has(id)) {
    selectedAgentIds.delete(id);
  } else {
    selectedAgentIds.add(id);
  }
  updateSelectionPanel();
  updateSelectionHighlight();
}

function startBoxSelect(e) {
  const { wx, wy, sx, sy } = worldAtScreen(e.clientX, e.clientY);
  selecting = true;
  selectStart = { sx, sy, wx, wy };
  elSelectBox.style.left = sx + "px";
  elSelectBox.style.top = sy + "px";
  elSelectBox.style.width = "0px";
  elSelectBox.style.height = "0px";
  elSelectBox.classList.add("active");
}

function updateBoxSelect(e) {
  if (!selecting || !selectStart) return;
  const { sx, sy } = worldAtScreen(e.clientX, e.clientY);
  const left = Math.min(selectStart.sx, sx);
  const top = Math.min(selectStart.sy, sy);
  const width = Math.abs(selectStart.sx - sx);
  const height = Math.abs(selectStart.sy - sy);
  elSelectBox.style.left = left + "px";
  elSelectBox.style.top = top + "px";
  elSelectBox.style.width = width + "px";
  elSelectBox.style.height = height + "px";
}

function finishBoxSelect(e) {
  if (!selecting || !selectStart) return;
  selecting = false;
  elSelectBox.classList.remove("active");

  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const x0 = Math.min(selectStart.wx, wx);
  const x1 = Math.max(selectStart.wx, wx);
  const y0 = Math.min(selectStart.wy, wy);
  const y1 = Math.max(selectStart.wy, wy);

  const ids = (lastSnap ? lastSnap.entities : [])
    .filter(
      (en) =>
        en.kind === "agent" &&
        en.x >= x0 &&
        en.x <= x1 &&
        en.y >= y0 &&
        en.y <= y1,
    )
    .map((en) => en.id);

  if (e.ctrlKey) {
    for (const id of ids) toggleSelectAgent(id);
  } else {
    setSelectedAgents(ids);
  }
  pushLog(`SELECTED ${selectedAgentIds.size} agents`);
}

function applyThemeChrome() {
  const u = theme.ui;
  document.documentElement.style.setProperty("--bg", u.bg);
  document.documentElement.style.setProperty("--hud", u.hud);
  document.documentElement.style.setProperty("--hud-muted", u.hudMuted);
  document.documentElement.style.setProperty("--online", u.online);
  document.documentElement.style.setProperty("--offline", u.offline);
  document.body.style.background = u.bg;
  elViewport.style.background = u.bg;
}

function setupThemeSelect() {
  elTheme.innerHTML = "";
  for (const t of THEMES) {
    const opt = document.createElement("option");
    opt.value = t.id;
    opt.textContent = t.name;
    if (t.id === theme.id) opt.selected = true;
    elTheme.appendChild(opt);
  }
  elTheme.addEventListener("change", () => {
    theme = getTheme(elTheme.value);
    localStorage.setItem(THEME_KEY, theme.id);
    applyThemeChrome();
    display = null;
    if (lastSnap) drawSnap(lastSnap);
  });
  applyThemeChrome();
}

function letterFg(letter) {
  return theme.letters[letter] || theme.letters.w || "#ccc";
}

function syncViewSize() {
  const cs = cellSize();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  viewCols = Math.max(8, Math.ceil(elViewport.clientWidth / cs) + 1);
  viewRows = Math.max(8, Math.ceil(elViewport.clientHeight / cs) + 1);
  if (mapW > 0) viewCols = Math.min(viewCols, mapW);
  if (mapH > 0) viewRows = Math.min(viewRows, mapH);

  const needNew =
    !display ||
    display._viewCols !== viewCols ||
    display._viewRows !== viewRows ||
    display._cellSize !== cs ||
    display._dpr !== dpr ||
    display._themeId !== theme.id;

  if (needNew) {
    elMap.innerHTML = "";
    display = new ROT.Display({
      width: viewCols,
      height: viewRows,
      fontSize: cs,
      fontFamily: "ui-monospace, 'SF Mono', Menlo, Consolas, monospace",
      bg: theme.void,
      fg: theme.ui.hud,
      forceSquareRatio: true,
      spacing: 1,
    });
    const canvas = display.getContainer();
    const lw = viewCols * cs;
    const lh = viewRows * cs;
    canvas.style.width = lw + "px";
    canvas.style.height = lh + "px";
    elMap.appendChild(canvas);
    elMap.style.width = lw + "px";
    elMap.style.height = lh + "px";
    display._viewCols = viewCols;
    display._viewRows = viewRows;
    display._cellSize = cs;
    display._dpr = dpr;
    display._themeId = theme.id;
  }
  clampCamera();
  updateHudCam();
  return display;
}

function clampCamera() {
  if (mapW <= 0 || mapH <= 0) return;
  cam.tx = Math.max(0, Math.min(mapW - viewCols, Math.floor(cam.tx)));
  cam.ty = Math.max(0, Math.min(mapH - viewRows, Math.floor(cam.ty)));
}

function updateHudCam() {
  elCam.textContent = `zoom ${cellSize()}px  ${cam.follow ? "FOLLOW" : "FREE"}  cam(${cam.tx},${cam.ty})`;
}

function updateModeHud() {
  if (!elMode) return;
  elMode.textContent = humanControl ? "PLAYER" : "MOCK";
  elMode.className = humanControl ? "mode-player" : "mode-mock";
}

function centerOnTile(tx, ty) {
  cam.tx = tx - Math.floor(viewCols / 2);
  cam.ty = ty - Math.floor(viewRows / 2);
  clampCamera();
  updateHudCam();
}

function zoomBy(delta, anchorScreenX, anchorScreenY) {
  const oldCs = cellSize();
  const oldZi = cam.zi;
  cam.zi = Math.max(0, Math.min(ZOOM_STEPS.length - 1, cam.zi + delta));
  if (cam.zi === oldZi) return;

  const rect = elViewport.getBoundingClientRect();
  const sx = anchorScreenX ?? rect.width / 2;
  const sy = anchorScreenY ?? rect.height / 2;
  const mapRect = elMap.getBoundingClientRect();
  const ox = sx - (mapRect.left - rect.left);
  const oy = sy - (mapRect.top - rect.top);
  const worldX = cam.tx + ox / oldCs;
  const worldY = cam.ty + oy / oldCs;

  cam.follow = false;
  syncViewSize();
  const cs = cellSize();
  cam.tx = worldX - ox / cs;
  cam.ty = worldY - oy / cs;
  clampCamera();
  if (lastSnap) drawSnap(lastSnap);
  updateHudCam();
}

function dimColor(hex, factor) {
  // simple darken for memory fog
  if (!hex || hex[0] !== "#" || (hex.length !== 7 && hex.length !== 4)) return hex;
  const full =
    hex.length === 4
      ? "#" + hex[1] + hex[1] + hex[2] + hex[2] + hex[3] + hex[3]
      : hex;
  const r = Math.floor(parseInt(full.slice(1, 3), 16) * factor);
  const g = Math.floor(parseInt(full.slice(3, 5), 16) * factor);
  const b = Math.floor(parseInt(full.slice(5, 7), 16) * factor);
  return (
    "#" +
    r.toString(16).padStart(2, "0") +
    g.toString(16).padStart(2, "0") +
    b.toString(16).padStart(2, "0")
  );
}

function drawSnap(snap) {
  if (!snap) return;
  mapW = snap.width;
  mapH = snap.height;
  const d = syncViewSize();
  clampCamera();

  const x0 = cam.tx;
  const y0 = cam.ty;
  const colorRows = snap.tile_colors || [];
  const visRows = snap.vision || [];

  d.clear();
  for (let vy = 0; vy < viewRows; vy++) {
    const wy = y0 + vy;
    const row = snap.tiles[wy] || "";
    const colorRow = colorRows[wy] || "";
    const visRow = visRows[wy] || "";
    for (let vx = 0; vx < viewCols; vx++) {
      const wx = x0 + vx;
      if (wy < 0 || wx < 0 || wy >= mapH || wx >= mapW) {
        d.draw(vx, vy, " ", theme.void, theme.void);
        continue;
      }
      const vch = visRow[wx] || "v"; // default visible if server old
      if (vch === " " || vch === "\0") {
        // unexplored darkness
        d.draw(vx, vy, " ", theme.void, theme.void);
        continue;
      }
      const ch = row[wx] || " ";
      const letter = colorRow[wx] || "w";
      let fg = letterFg(letter);
      let bg = theme.cellBg(letter, ch);
      if (vch === "m") {
        // remembered: dim fog of war
        fg = dimColor(fg, 0.38);
        bg = dimColor(bg, 0.45);
      }
      d.draw(vx, vy, ch, fg, bg);
    }
  }

  const ents = snap.entities || [];
  const e = theme.entities;
  const SELECT_BG = theme.selection || "#003333";
  const trackedIds = new Set(
    tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  for (const ent of ents) {
    const vx = ent.x - x0;
    const vy = ent.y - y0;
    if (vx < 0 || vy < 0 || vx >= viewCols || vy >= viewRows) continue;
    if (ent.kind === "agent") {
      if (!trackedIds.has(ent.id)) continue; // only show explicitly tracked agents
      // color by tracker if registered
      const tr = tracked.find((t) => t.agent_id === ent.id);
      const fg = tr ? tr.color : e.agent;
      const glyph = ent.name ? ent.name[0].toUpperCase() : "@";
      let bg = e.entityBg;
      if (selectedAgentIds.has(ent.id)) {
        bg = SELECT_BG;
      }
      d.draw(vx, vy, glyph === "@" || !/[A-Za-z]/.test(glyph) ? "@" : glyph, fg, bg);
    } else {
      const fg = e[ent.kind] || e.iron;
      d.draw(vx, vy, ent.glyph || "?", fg, e.entityBg);
    }
  }
  // tracked markers even if name missing
  for (const t of tracked) {
    if (t.x == null || t.y == null) continue;
    const vx = t.x - x0;
    const vy = t.y - y0;
    if (vx < 0 || vy < 0 || vx >= viewCols || vy >= viewRows) continue;
    // ring: redraw @ with track color (already done if agent_id match)
    if (followToken === t.token) {
      // highlight underfoot with accent
      d.draw(vx, vy, "@", t.color, "#1e1e1e");
    }
  }

  elMap.style.left = "0px";
  elMap.style.top = "0px";
  const lw = viewCols * cellSize();
  const lh = viewRows * cellSize();
  elMap.style.marginLeft =
    lw < elViewport.clientWidth
      ? Math.floor((elViewport.clientWidth - lw) / 2) + "px"
      : "0px";
  elMap.style.marginTop =
    lh < elViewport.clientHeight
      ? Math.floor((elViewport.clientHeight - lh) / 2) + "px"
      : "0px";
}

function inspectToken() {
  return followToken || (tracked[0] && tracked[0].token);
}

function showInspectPopup(title, html) {
  if (!elInspectPopup || !elInspectTitle || !elInspectBody) return;
  elInspectTitle.textContent = title;
  elInspectBody.innerHTML = html;
  elInspectPopup.classList.add("visible");
}

function hideInspectPopup() {
  if (!elInspectPopup) return;
  elInspectPopup.classList.remove("visible");
}

function renderKV(obj, skip = []) {
  const rows = [];
  for (const [k, v] of Object.entries(obj)) {
    if (skip.includes(k)) continue;
    if (v === null || v === undefined) continue;
    let display = v;
    if (Array.isArray(v)) {
      display = v
        .map((it) =>
          typeof it === "object"
            ? Object.entries(it)
                .map(([kk, vv]) => `${kk}:${vv}`)
                .join(" ")
            : String(it),
        )
        .join("<br>");
    } else if (typeof v === "object") {
      display = renderKV(v);
    } else {
      display = String(v);
    }
    rows.push(`<tr><td>${k}</td><td>${display}</td></tr>`);
  }
  return `<table>${rows.join("")}</table>`;
}

function renderEntityInspect(e) {
  const kind = e.kind || "unknown";
  const glyph = e.glyph || "?";
  const title = `[${glyph}] ${kind}${e.name ? " — " + e.name : ""}`;
  const html =
    `<div class="muted">position (${e.x}, ${e.y}) · id ${e.id}</div>` +
    renderKV(e, ["id", "x", "y", "glyph"]);
  showInspectPopup(title, html);
}

function renderCellInspect(c) {
  const title = `[${c.glyph || " "}] ${c.name || "cell"}`;
  const html =
    `<div class="muted">position (${c.x}, ${c.y}) · feat ${c.feat_id}</div>` +
    renderKV(c, ["x", "y", "glyph", "name", "feat_id"]);
  showInspectPopup(title, html);
}

async function fetchEntityInspect(id) {
  const token = inspectToken();
  if (!token) {
    pushLog("INSPECT: track a token first");
    return;
  }
  try {
    const r = await fetch(
      `/api/entity?id=${id}&token=` + encodeURIComponent(token),
    );
    const d = await r.json();
    if (!d.ok) {
      pushLog("INSPECT: " + (d.reason || "failed"));
      return;
    }
    renderEntityInspect(d.entity);
  } catch (_) {
    pushLog("INSPECT: network");
  }
}

async function fetchCellInspect(wx, wy) {
  const token = inspectToken();
  if (!token) {
    pushLog("INSPECT: track a token first");
    return;
  }
  try {
    const r = await fetch(
      `/api/cell?x=${wx}&y=${wy}&token=` + encodeURIComponent(token),
    );
    const d = await r.json();
    if (!d.ok) {
      pushLog("INSPECT: " + (d.reason || "failed"));
      return;
    }
    renderCellInspect(d.cell);
  } catch (_) {
    pushLog("INSPECT: network");
  }
}

function handleInspectClick(e, button) {
  if (!lastSnap || mapW <= 0 || mapH <= 0) return;
  const mapRect = elMap.getBoundingClientRect();
  const cs = cellSize();
  const sx = e.clientX - mapRect.left;
  const sy = e.clientY - mapRect.top;
  const wx = Math.floor(cam.tx + sx / cs);
  const wy = Math.floor(cam.ty + sy / cs);
  if (wx < 0 || wy < 0 || wx >= mapW || wy >= mapH) return;

  if (button === 2) {
    fetchCellInspect(wx, wy);
    return;
  }

  const ents = (lastSnap.entities || []).filter(
    (en) => en.x === wx && en.y === wy,
  );
  if (ents.length) {
    // prefer agents, then monsters, then items, then resources/buildings
    const order = ["agent", "monster", "item", "tree", "iron", "hut"];
    const sorted = ents.slice().sort((a, b) => {
      const ia = order.indexOf(a.kind);
      const ib = order.indexOf(b.kind);
      if (ia !== -1 && ib !== -1) return ia - ib;
      if (ia !== -1) return -1;
      if (ib !== -1) return 1;
      return 0;
    });
    fetchEntityInspect(sorted[0].id);
  } else {
    fetchCellInspect(wx, wy);
  }
}

function handleSelectClick(e) {
  if (!lastSnap || mapW <= 0 || mapH <= 0) return;
  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const ent = (lastSnap.entities || []).find(
    (en) => en.kind === "agent" && en.x === wx && en.y === wy,
  );
  if (ent) {
    toggleSelectAgent(ent.id);
  }
}

function pushLog(msg) {
  if (!elLog) return;
  const line = document.createElement("div");
  line.textContent = "> " + msg;
  // keep CSS ::before LOG title; only manage message lines
  elLog.insertBefore(line, elLog.firstChild);
  while (elLog.querySelectorAll("div").length > 10) {
    const nodes = elLog.querySelectorAll("div");
    elLog.removeChild(nodes[nodes.length - 1]);
  }
}

function formatEvents(events) {
  if (!events || !events.length) return;
  for (const ev of events.slice(-4)) {
    const t = ev.type || "?";
    if (t === "moved") pushLog(`→ (${ev.to[0]},${ev.to[1]})`);
    else if (t === "move_failed") pushLog(`✗ ${ev.reason}`);
    else if (t === "harvested") pushLog(`✂ ${ev.kind} +${ev.amount}`);
    else if (t === "built") pushLog(`⌂ hut @(${ev.at[0]},${ev.at[1]})`);
    else if (t === "trap_triggered") pushLog(`⚠ trap ${ev.name} -${ev.damage}`);
    else if (t === "terrain_damage") pushLog(`♨ ${ev.kind} -${ev.damage} hp=${ev.hp}`);
    else if (t === "door_opened") pushLog(`开门 (${ev.at[0]},${ev.at[1]})`);
    else if (t === "door_closed") pushLog(`关门 (${ev.at[0]},${ev.at[1]})`);
    else if (t === "level_changed") pushLog(`↕ depth=${ev.depth}`);
    else if (t === "item_picked_up") pushLog(`拾取 ${ev.name}`);
    else if (t === "item_dropped") pushLog(`丢下 ${ev.name}`);
    else if (t === "monster_attacked") pushLog(`⚔ ${ev.name} -${ev.damage} hp=${ev.target_hp}`);
    else if (t === "player_attacked") pushLog(`击中 ${ev.name} -${ev.damage} hp=${ev.target_hp}`);
    else if (t === "monster_killed") pushLog(`击杀 ${ev.name}`);
    else if (t === "dug") pushLog(`挖 (${ev.at[0]},${ev.at[1]}) → pack`);
    else if (t === "scooped") pushLog(`铲 (${ev.at[0]},${ev.at[1]}) → pack`);
    else if (t === "placed") pushLog(`放 feat=${ev.feat} @(${ev.at[0]},${ev.at[1]})`);
    else if (t === "crafted") pushLog(`合成 ${ev.recipe}`);
    else if (t === "planted") pushLog(`种植 @(${ev.at[0]},${ev.at[1]})`);
    else if (t === "deconstructed") pushLog(`拆 hut +${ev.wood} wood`);
    else if (t === "rested") pushLog(`休 +${ev.healed} hp=${ev.hp}`);
  }
}

function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;
  const prevTick = lastSnap ? lastSnap.tick : -1;
  lastSnap = snap;

  const ents = snap.entities || [];
  const trackedIds = new Set(
    tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  const agents = ents.filter(
    (e) => e.kind === "agent" && trackedIds.has(e.id),
  );
  // sync tracked poses from snapshot when agent_id known
  for (const t of tracked) {
    if (t.agent_id == null) continue;
    const a = agents.find((e) => e.id === t.agent_id);
    if (a) {
      t.x = a.x;
      t.y = a.y;
      if (a.name) t.name = a.name;
    }
  }
  const agent = focusAgent();
  const trees = ents.filter((e) => e.kind === "tree").length;
  const irons = ents.filter((e) => e.kind === "iron").length;
  const huts = ents.filter((e) => e.kind === "hut").length;
  const mons = ents.filter((e) => e.kind === "monster").length;
  const items = ents.filter((e) => e.kind === "item").length;
  const hp = agent && agent.hp != null ? ` hp=${agent.hp}/${agent.max_hp ?? "?"}` : "";
  const pack =
    agent && agent.items && agent.items.length ? ` pack=${agent.items.length}` : "";
  const interactionsSrc = lastMe && lastMe.interactions ? lastMe.interactions : snap.interactions || [];
  const acts = interactionsSrc
    .slice(0, 3)
    .map((i) => i.verb)
    .join(",");
  const actHud = acts ? ` [${acts}]` : "";
  const names = agents
    .map((a) => a.name || `@${a.id}`)
    .slice(0, 4)
    .join(",");
  elInfo.textContent = `t=${snap.tick} ${snap.width}x${snap.height} agents=${agents.length}${
    agent ? ` focus=${agent.name || agent.id} @(${agent.x},${agent.y})${hp}${pack}` : ""
  } T=${trees} H=${huts} M=${mons}${actHud}${names ? " {" + names + "}" : ""}`;

  if (snap.tick !== prevTick && snap.recent_events) {
    formatEvents(snap.recent_events.filter((e) => e.type !== "tick_started"));
  }
  if (snap.tick !== prevTick && snap.tick % 5 === 0) {
    refreshTracked();
    refreshMe();
  }

  if (cam.follow) {
    mapW = snap.width;
    mapH = snap.height;
    syncViewSize();
    if (agent) centerOnTile(agent.x, agent.y);
  }
  drawSnap(snap);
}

/** Send action to kernel (WS preferred, HTTP fallback). */
function sendAction(action) {
  const token = followToken || (tracked[0] && tracked[0].token);
  if (!token) {
    pushLog("NO TRACKED AGENT — paste a token");
    return;
  }
  const agent = focusAgent();
  const body = {
    type: "action",
    token,
    agent_id: agent ? agent.id : undefined,
    tick: lastSnap ? lastSnap.tick : undefined,
    action,
  };
  humanControl = true;
  updateModeHud();
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(body));
    return;
  }
  fetch("/api/action", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      token,
      agent_id: body.agent_id,
      tick: body.tick,
      action,
    }),
  }).catch(() => {});
}

function setHumanControl(on) {
  humanControl = on;
  updateModeHud();
  const msg = { type: "control", human_control: on };
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  } else {
    fetch("/api/control", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ human_control: on }),
    }).catch(() => {});
  }
}

function dirFromKey(e) {
  // arrow / wasd / hjkl / numpad
  switch (e.key) {
    case "ArrowLeft":
    case "a":
    case "A":
    case "h":
    case "H":
    case "4":
      return [-1, 0];
    case "ArrowRight":
    case "d":
    case "D":
    case "l":
    case "L":
    case "6":
      return [1, 0];
    case "ArrowUp":
    case "w":
    case "W":
    case "k":
    case "K":
    case "8":
      return [0, -1];
    case "ArrowDown":
    case "s":
    case "S":
    case "j":
    case "J":
    case "2":
      return [0, 1];
    default:
      return null;
  }
}

let dragging = false;
let dragLast = null;
let accumX = 0;
let accumY = 0;
let dragPixelDist = 0;
let mouseDownAt = null;

elViewport.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const rect = elViewport.getBoundingClientRect();
    zoomBy(e.deltaY < 0 ? 1 : -1, e.clientX - rect.left, e.clientY - rect.top);
  },
  { passive: false },
);

elViewport.addEventListener("mousedown", (e) => {
  if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;

  if (e.shiftKey && e.button === 0) {
    e.preventDefault();
    startBoxSelect(e);
    return;
  }

  dragging = true;
  dragLast = { x: e.clientX, y: e.clientY };
  accumX = 0;
  accumY = 0;
  dragPixelDist = 0;
  mouseDownAt = {
    x: e.clientX,
    y: e.clientY,
    t: performance.now(),
    button: e.button,
    shiftKey: e.shiftKey,
    ctrlKey: e.ctrlKey,
  };
  elViewport.classList.add("dragging");
  cam.follow = false;
  e.preventDefault();
});

window.addEventListener("mousemove", (e) => {
  if (selecting) {
    updateBoxSelect(e);
    return;
  }
  if (!dragging || !dragLast) return;
  const dx = e.clientX - dragLast.x;
  const dy = e.clientY - dragLast.y;
  dragLast = { x: e.clientX, y: e.clientY };
  dragPixelDist += Math.abs(dx) + Math.abs(dy);
  accumX += dx;
  accumY += dy;
  const cs = cellSize();
  while (accumX >= cs) {
    cam.tx -= 1;
    accumX -= cs;
  }
  while (accumX <= -cs) {
    cam.tx += 1;
    accumX += cs;
  }
  while (accumY >= cs) {
    cam.ty -= 1;
    accumY -= cs;
  }
  while (accumY <= -cs) {
    cam.ty += 1;
    accumY += cs;
  }
  clampCamera();
  if (lastSnap) drawSnap(lastSnap);
  updateHudCam();
});

window.addEventListener("mouseup", (e) => {
  if (selecting) {
    finishBoxSelect(e);
    return;
  }
  if (!dragging) return;
  dragging = false;
  dragLast = null;
  elViewport.classList.remove("dragging");
  const down = mouseDownAt;
  mouseDownAt = null;
  if (down) {
    const dt = performance.now() - down.t;
    if (dragPixelDist < 6 && dt < 450) {
      if (down.ctrlKey) {
        handleSelectClick(e);
      } else {
        handleInspectClick(e, down.button);
      }
    }
  }
});

elViewport.addEventListener("contextmenu", (e) => e.preventDefault());

window.addEventListener("keydown", (e) => {
  // ignore when typing in inputs
  if (e.target && (e.target.tagName === "INPUT" || e.target.tagName === "SELECT" || e.target.tagName === "TEXTAREA")) {
    return;
  }

  if (
    e.key === "Escape" &&
    elInspectPopup &&
    elInspectPopup.classList.contains("visible")
  ) {
    hideInspectPopup();
    e.preventDefault();
    return;
  }
  if (e.code === "Space") {
    e.preventDefault();
    cam.follow = true;
    if (lastSnap) {
      const agent = focusAgent();
      if (agent) {
        centerOnTile(agent.x, agent.y);
        drawSnap(lastSnap);
      }
    }
    return;
  }
  if (e.key === "+" || e.key === "=") {
    zoomBy(1, elViewport.clientWidth / 2, elViewport.clientHeight / 2);
    return;
  }
  if (e.key === "-" || e.key === "_") {
    zoomBy(-1, elViewport.clientWidth / 2, elViewport.clientHeight / 2);
    return;
  }
  if (e.key === "r" || e.key === "R") {
    cam.zi = 4;
    cam.follow = true;
    syncViewSize();
    if (lastSnap) {
      const agent = focusAgent();
      if (agent) centerOnTile(agent.x, agent.y);
      drawSnap(lastSnap);
    }
    return;
  }

  // resume mock auto-play
  if (e.key === "m" || e.key === "M") {
    e.preventDefault();
    setHumanControl(false);
    pushLog("MOCK 自动");
    return;
  }

  // --- player sandbox: only move / interact / drop / rest / idle ---
  if (pendingDirCmd) {
    const d = dirFromKey(e);
    if (d) {
      e.preventDefault();
      sendAction({
        type: "interact",
        dx: d[0],
        dy: d[1],
        verb: pendingDirCmd,
      });
      pushLog(`interact ${pendingDirCmd} ${d[0]},${d[1]}`);
      pendingDirCmd = null;
    } else if (e.key === "Escape") {
      pendingDirCmd = null;
      pushLog("取消");
    }
    return;
  }

  // verb + direction (options also listed in snapshot.interactions)
  if (e.key === "o" || e.key === "O") {
    e.preventDefault();
    pendingDirCmd = "open";
    pushLog("open + 方向");
    return;
  }
  if (e.key === "c" || e.key === "C") {
    e.preventDefault();
    pendingDirCmd = "close";
    pushLog("close + 方向");
    return;
  }
  if (e.key === "f" || e.key === "F") {
    e.preventDefault();
    pendingDirCmd = "attack";
    pushLog("attack + 方向");
    return;
  }
  if (e.key === "t" || e.key === "T") {
    e.preventDefault();
    pendingDirCmd = "dig";
    pushLog("dig + 方向");
    return;
  }
  if (e.key === "v" || e.key === "V") {
    e.preventDefault();
    pendingDirCmd = "place";
    pushLog("place + 方向");
    return;
  }
  if (e.key === "u" || e.key === "U") {
    e.preventDefault();
    pendingDirCmd = "scoop";
    pushLog("scoop + 方向 (0,0 用 g)");
    return;
  }
  if (e.key === "n" || e.key === "N") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "plant" });
    pushLog("plant");
    return;
  }
  if (e.key === "x" || e.key === "X") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "deconstruct" });
    pushLog("deconstruct");
    return;
  }
  if (e.key === "y" || e.key === "Y") {
    e.preventDefault();
    // craft first available recipe from focused agent's interactions
    const interactions = (lastMe && lastMe.interactions) || (lastSnap && lastSnap.interactions) || [];
    const craft = interactions.find((i) => i.verb === "craft");
    if (craft && craft.recipe) {
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "craft", recipe: craft.recipe });
      pushLog("craft " + craft.recipe);
    } else {
      pushLog("无可用配方");
    }
    return;
  }

  // underfoot interact (default verb / single option)
  if (e.key === "g" || e.key === "G" || e.key === "Enter") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0 });
    pushLog("interact here");
    return;
  }
  if (e.key === "b" || e.key === "B") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "build" });
    pushLog("build");
    return;
  }
  if (e.key === "," || e.key === "p" || e.key === "P") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "pickup" });
    pushLog("pickup");
    return;
  }
  if (e.key === "z" || e.key === "Z") {
    e.preventDefault();
    sendAction({ type: "rest" });
    pushLog("rest");
    return;
  }
  if ((e.key === "d" || e.key === "D") && e.shiftKey) {
    e.preventDefault();
    sendAction({ type: "drop", index: 0 });
    pushLog("drop 0");
    return;
  }
  if (e.key === "." || e.key === "5") {
    e.preventDefault();
    sendAction({ type: "idle" });
    pushLog("idle");
    return;
  }
  if (e.key === ">") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "descend" });
    pushLog("descend");
    return;
  }
  if (e.key === "<") {
    e.preventDefault();
    sendAction({ type: "interact", dx: 0, dy: 0, verb: "ascend" });
    pushLog("ascend");
    return;
  }

  // movement (always player action when human keys used)
  const d = dirFromKey(e);
  if (d) {
    // In FREE cam mode, Shift+arrows pan camera instead of moving agent
    if (!cam.follow && e.shiftKey && (e.key.startsWith("Arrow") || ["w","a","s","d","W","A","S","D"].includes(e.key))) {
      e.preventDefault();
      cam.tx += d[0] * 2;
      cam.ty += d[1] * 2;
      clampCamera();
      if (lastSnap) drawSnap(lastSnap);
      updateHudCam();
      return;
    }
    e.preventDefault();
    cam.follow = true;
    sendAction({ type: "move", dx: d[0], dy: d[1] });
  }
});

window.addEventListener("resize", () => {
  if (lastSnap) {
    if (cam.follow) {
      const agent = focusAgent();
      syncViewSize();
      if (agent) centerOnTile(agent.x, agent.y);
    } else {
      syncViewSize();
    }
    drawSnap(lastSnap);
  }
});

function connect() {
  elStatus.textContent = "connecting";
  elStatus.className = "offline";
  ws = new WebSocket(WS_URL);
  ws.onopen = () => {
    elStatus.textContent = "live";
    elStatus.className = "online";
    sendSubscribe();
  };
  ws.onclose = () => {
    elStatus.textContent = "offline";
    elStatus.className = "offline";
    ws = null;
    setTimeout(connect, 1200);
  };
  ws.onerror = () => ws && ws.close();
  ws.onmessage = (ev) => {
    try {
      applySnapshot(JSON.parse(ev.data));
    } catch (_) {}
  };
}

if (elTokenAdd) {
  elTokenAdd.addEventListener("click", () => {
    addToken(elTokenInput && elTokenInput.value);
    if (elTokenInput) elTokenInput.value = "";
  });
}
if (elTokenClear) {
  elTokenClear.addEventListener("click", () => {
    clearTracked();
  });
}
if (elTokenInput) {
  elTokenInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addToken(elTokenInput.value);
      elTokenInput.value = "";
    }
  });
}
if (elBtnFollow) {
  elBtnFollow.addEventListener("click", () => {
    cam.follow = true;
    pushLog("FOLLOW ON");
    if (lastSnap) applySnapshot(lastSnap);
  });
}
if (elBtnMock) {
  elBtnMock.addEventListener("click", () => {
    setHumanControl(false);
    pushLog("MOCK");
  });
}

if (elInspectClose) {
  elInspectClose.addEventListener("click", (e) => {
    e.stopPropagation();
    hideInspectPopup();
  });
}

if (elSelPreset) {
  elSelPreset.addEventListener("change", () => {
    const id = elSelPreset.value;
    if (!id) return;
    const p = loadPresets().find((x) => x.id === id);
    if (p && elSelText) elSelText.value = p.text;
  });
}
if (elSelPresetSave) {
  elSelPresetSave.addEventListener("click", () => {
    const name = (elSelPresetName && elSelPresetName.value || "").trim();
    const text = (elSelText && elSelText.value || "").trim();
    if (!name || !text) {
      pushLog("PRESET: need name and text");
      return;
    }
    const presets = loadPresets();
    presets.push({ id: Date.now().toString(36), name, text });
    savePresets(presets);
    renderPresets();
    if (elSelPresetName) elSelPresetName.value = "";
    pushLog(`PRESET saved: ${name}`);
  });
}
if (elSelPresetDel) {
  elSelPresetDel.addEventListener("click", () => {
    const id = elSelPreset.value;
    if (!id) return;
    const presets = loadPresets().filter((p) => p.id !== id);
    savePresets(presets);
    renderPresets();
    if (elSelText) elSelText.value = "";
    pushLog("PRESET deleted");
  });
}
if (elSelSend) {
  elSelSend.addEventListener("click", () => {
    sendPromptToSelected(elSelText ? elSelText.value : "");
  });
}

setupThemeSelect();
updateModeHud();
renderPresets();
updateSelectionPanel();
renderTracker();
refreshTracked();
connect();
