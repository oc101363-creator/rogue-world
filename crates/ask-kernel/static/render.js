/* ASK viewer — rendering: map/entities, camera, themes, panels, inspect popup.
 * Imports state only; calls into net lazily where the tracker UI needs it. */

import { el, S, ZOOM_STEPS, THEME_KEY, saveTracked, loadPresets, agentName } from "./state.js";
import { THEMES, getTheme } from "./themes.js";
import { decodeFeatIds, lookForFeat, lookForEntity, materialColor } from "./art.js";
import { sendSubscribe } from "./net.js";

// ---------------------------------------------------------------- selectors

export function focusAgent() {
  if (!S.lastSnap) return null;
  const trackedIds = new Set(
    S.tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  const agents = (S.lastSnap.entities || []).filter(
    (e) => e.kind === "agent" && trackedIds.has(e.id),
  );
  if (S.followToken) {
    const t = S.tracked.find((x) => x.token === S.followToken);
    if (t && t.agent_id != null) {
      return agents.find((e) => e.id === t.agent_id) || null;
    }
  }
  return agents[0] || null;
}

export function agentsInWorldRect(x0, y0, x1, y1) {
  const loX = Math.min(x0, x1);
  const hiX = Math.max(x0, x1);
  const loY = Math.min(y0, y1);
  const hiY = Math.max(y0, y1);
  return (S.lastSnap ? S.lastSnap.entities : [])
    .filter(
      (en) =>
        en.kind === "agent" &&
        en.x >= loX &&
        en.x <= hiX &&
        en.y >= loY &&
        en.y <= hiY,
    )
    .map((en) => en.id);
}

export function visibleAgentIds() {
  if (!S.lastSnap) return [];
  const visRows = S.lastSnap.vision || [];
  return (S.lastSnap.entities || [])
    .filter((en) => {
      if (en.kind !== "agent") return false;
      const row = visRows[en.y] || "";
      const ch = row[en.x] || " ";
      // currently lit FOV only (not fog-of-war memory)
      return ch === "v";
    })
    .map((en) => en.id);
}

// ---------------------------------------------------------------- log

export function pushLog(msg) {
  if (!el.log) return;
  const line = document.createElement("div");
  line.textContent = "> " + msg;
  el.log.insertBefore(line, el.log.firstChild);
  while (el.log.querySelectorAll("div").length > 10) {
    const nodes = el.log.querySelectorAll("div");
    el.log.removeChild(nodes[nodes.length - 1]);
  }
}

export function formatEvents(events) {
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
    else if (t === "agent_died") pushLog(`☠ 倒下 @(${ev.at[0]},${ev.at[1]})`);
    else if (t === "agent_respawned") pushLog(`✚ 重生 @(${ev.at[0]},${ev.at[1]})`);
    else if (t === "terrain_changed") pushLog(`≋ ${ev.cause} (${ev.at[0]},${ev.at[1]})`);
    else if (t === "consumed") pushLog(`吃 ${ev.label} hp=${ev.hp}`);
  }
}

// ---------------------------------------------------------------- panels

export function renderTracker() {
  if (!el.trackerList) return;
  el.trackerList.innerHTML = "";
  S.tracked.forEach((t, i) => {
    const div = document.createElement("div");
    div.className = "track-item" + (S.followToken === t.token ? " active" : "");
    div.innerHTML =
      `<button type="button" class="rm" data-i="${i}" title="remove">[x]</button>` +
      `<div class="name" style="color:${t.color}">@ ${t.name || "agent"}</div>` +
      `<div class="meta">id=${t.agent_id ?? "?"}  @(${t.x ?? "?"},${t.y ?? "?"})</div>` +
      `<div class="tok">${t.token.slice(0, 18)}…</div>`;
    div.addEventListener("click", (e) => {
      if (e.target.classList.contains("rm")) return;
      S.followToken = t.token;
      S.cam.follow = true;
      sendSubscribe();
      if (t.x != null && t.y != null) {
        centerOnTile(t.x, t.y);
        if (S.lastSnap) drawSnap(S.lastSnap);
      }
      renderTracker();
      pushLog(`FOLLOW ${t.name || t.token.slice(0, 12)}`);
    });
    div.querySelector(".rm").addEventListener("click", (e) => {
      e.stopPropagation();
      S.tracked.splice(i, 1);
      if (S.followToken === t.token)
        S.followToken = S.tracked.length ? S.tracked[0].token : null;
      saveTracked();
      renderTracker();
      sendSubscribe();
    });
    el.trackerList.appendChild(div);
  });
  if (el.trackerHint) el.trackerHint.textContent = `${S.tracked.length} tracked (saved locally)`;
}

export function renderPresets() {
  if (!el.selPreset) return;
  const presets = loadPresets();
  el.selPreset.innerHTML = "";
  const none = document.createElement("option");
  none.value = "";
  none.textContent = "-- preset --";
  el.selPreset.appendChild(none);
  for (const p of presets) {
    const opt = document.createElement("option");
    opt.value = p.id;
    opt.textContent = p.name;
    el.selPreset.appendChild(opt);
  }
}


export function updateSelectionPanel() {
  if (!el.selCount) return;
  el.selCount.textContent = `${S.selectedAgentIds.size} agents selected`;
  renderSelectionChips();
}

/** Recipient chips: who's in the broadcast — named, removable, and dimmed
 * when out of live FOV (a send to them will be rejected `not_visible`). */
export function renderSelectionChips() {
  if (!el.selChips) return;
  el.selChips.innerHTML = "";
  const visible = new Set(visibleAgentIds());
  for (const id of S.selectedAgentIds) {
    const chip = document.createElement("span");
    chip.className = "sel-chip" + (visible.has(id) ? "" : " out-of-fov");
    chip.title = visible.has(id) ? "in FOV" : "out of FOV — send will be rejected";
    chip.textContent = agentName(id) + " ";
    const rm = document.createElement("button");
    rm.type = "button";
    rm.className = "rm";
    rm.textContent = "[x]";
    rm.addEventListener("click", (e) => {
      e.stopPropagation();
      S.selectedAgentIds.delete(id);
      updateSelectionPanel();
      updateSelectionHighlight();
    });
    chip.appendChild(rm);
    el.selChips.appendChild(chip);
  }
}

/** Per-target delivery rows under SEND: sent → read(tick), or ✗ reason. */
export function renderDelivery() {
  if (!el.selDelivery) return;
  el.selDelivery.innerHTML = "";
  for (const d of S.delivery) {
    const row = document.createElement("div");
    row.className = "delivery-row";
    if (!d.ok) {
      row.textContent = `✗ ${agentName(d.agent)} — ${d.reason}`;
      row.classList.add("delivery-fail");
    } else if (d.read_tick != null) {
      row.textContent = `✓ ${agentName(d.agent)} — read (tick ${d.read_tick})`;
      row.classList.add("delivery-read");
    } else {
      row.textContent = `… ${agentName(d.agent)} — unread`;
      row.classList.add("delivery-pending");
    }
    el.selDelivery.appendChild(row);
  }
}

export function updateSelectionHighlight() {
  if (S.lastSnap) drawSnap(S.lastSnap);
}

// ---------------------------------------------------------------- theme

export function applyThemeChrome() {
  const u = S.theme.ui;
  document.documentElement.style.setProperty("--bg", u.bg);
  document.documentElement.style.setProperty("--hud", u.hud);
  document.documentElement.style.setProperty("--hud-muted", u.hudMuted);
  document.documentElement.style.setProperty("--online", u.online);
  document.documentElement.style.setProperty("--offline", u.offline);
  document.body.style.background = u.bg;
  el.viewport.style.background = u.bg;
}

export function setupThemeSelect() {
  el.theme.innerHTML = "";
  for (const t of THEMES) {
    const opt = document.createElement("option");
    opt.value = t.id;
    opt.textContent = t.name;
    if (t.id === S.theme.id) opt.selected = true;
    el.theme.appendChild(opt);
  }
  el.theme.addEventListener("change", () => {
    S.theme = getTheme(el.theme.value);
    localStorage.setItem(THEME_KEY, S.theme.id);
    applyThemeChrome();
    S.display = null;
    if (S.lastSnap) drawSnap(S.lastSnap);
  });
  applyThemeChrome();
}

// ---------------------------------------------------------------- camera

export function cellSize() {
  return ZOOM_STEPS[S.cam.zi];
}

export function worldAtScreen(clientX, clientY) {
  const mapRect = el.map.getBoundingClientRect();
  const cs = cellSize();
  const mx = clientX - mapRect.left;
  const my = clientY - mapRect.top;
  return {
    wx: Math.floor(S.cam.tx + mx / cs),
    wy: Math.floor(S.cam.ty + my / cs),
    mx,
    my,
  };
}

export function viewportAtScreen(clientX, clientY) {
  const rect = el.viewport.getBoundingClientRect();
  return {
    sx: clientX - rect.left,
    sy: clientY - rect.top,
  };
}

export function syncViewSize() {
  const cs = cellSize();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  S.viewCols = Math.max(8, Math.ceil(el.viewport.clientWidth / cs) + 1);
  S.viewRows = Math.max(8, Math.ceil(el.viewport.clientHeight / cs) + 1);
  if (S.mapW > 0) S.viewCols = Math.min(S.viewCols, S.mapW);
  if (S.mapH > 0) S.viewRows = Math.min(S.viewRows, S.mapH);

  const needNew =
    !S.display ||
    S.display._viewCols !== S.viewCols ||
    S.display._viewRows !== S.viewRows ||
    S.display._cellSize !== cs ||
    S.display._dpr !== dpr ||
    S.display._themeId !== S.theme.id;

  if (needNew) {
    el.map.innerHTML = "";
    S.display = new ROT.Display({
      width: S.viewCols,
      height: S.viewRows,
      fontSize: cs,
      fontFamily: "ui-monospace, 'SF Mono', Menlo, Consolas, monospace",
      bg: S.theme.void,
      fg: S.theme.ui.hud,
      forceSquareRatio: true,
      spacing: 1,
    });
    const canvas = S.display.getContainer();
    const lw = S.viewCols * cs;
    const lh = S.viewRows * cs;
    canvas.style.width = lw + "px";
    canvas.style.height = lh + "px";
    el.map.appendChild(canvas);
    el.map.style.width = lw + "px";
    el.map.style.height = lh + "px";
    S.display._viewCols = S.viewCols;
    S.display._viewRows = S.viewRows;
    S.display._cellSize = cs;
    S.display._dpr = dpr;
    S.display._themeId = S.theme.id;
  }
  clampCamera();
  updateHudCam();
  return S.display;
}

export function clampCamera() {
  if (S.mapW <= 0 || S.mapH <= 0) return;
  S.cam.tx = Math.max(0, Math.min(S.mapW - S.viewCols, Math.floor(S.cam.tx)));
  S.cam.ty = Math.max(0, Math.min(S.mapH - S.viewRows, Math.floor(S.cam.ty)));
}

export function updateHudCam() {
  el.cam.textContent = `zoom ${cellSize()}px  ${S.cam.follow ? "FOLLOW" : "FREE"}  cam(${S.cam.tx},${S.cam.ty})`;
}

export function updateModeHud() {
  if (!el.mode) return;
  el.mode.textContent = S.humanControl ? "PLAYER" : "MOCK";
  el.mode.className = S.humanControl ? "mode-player" : "mode-mock";
}

export function centerOnTile(tx, ty) {
  S.cam.tx = tx - Math.floor(S.viewCols / 2);
  S.cam.ty = ty - Math.floor(S.viewRows / 2);
  clampCamera();
  updateHudCam();
}

export function zoomBy(delta, anchorScreenX, anchorScreenY) {
  const oldCs = cellSize();
  const oldZi = S.cam.zi;
  S.cam.zi = Math.max(0, Math.min(ZOOM_STEPS.length - 1, S.cam.zi + delta));
  if (S.cam.zi === oldZi) return;

  const rect = el.viewport.getBoundingClientRect();
  const sx = anchorScreenX ?? rect.width / 2;
  const sy = anchorScreenY ?? rect.height / 2;
  const mapRect = el.map.getBoundingClientRect();
  const ox = sx - (mapRect.left - rect.left);
  const oy = sy - (mapRect.top - rect.top);
  const worldX = S.cam.tx + ox / oldCs;
  const worldY = S.cam.ty + oy / oldCs;

  S.cam.follow = false;
  syncViewSize();
  const cs = cellSize();
  S.cam.tx = worldX - ox / cs;
  S.cam.ty = worldY - oy / cs;
  clampCamera();
  if (S.lastSnap) drawSnap(S.lastSnap);
  updateHudCam();
}

// ---------------------------------------------------------------- map draw

export function dimColor(hex, factor) {
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

const MATERIAL_BG = {
  aquifer: "#001028",
  water: "#001028",
  water_deep: "#000a20",
  magma: "#1a0800",
  fire: "#1a0800",
  plant: "#0a1008",
  organic: "#0a1008",
  brake: "#0a1008",
  earth: "#120c08",
  wood: "#120c08",
  door: "#120c08",
  granite: "#101010",
  stone_dark: "#101010",
  stone_light: "#101010",
  floor: "#0a0a0c",
  basalt: "#0a0a0c",
  gold: "#1a1400",
  flower: "#100818",
  magic: "#100818",
  crystal: "#100818",
  trap: "#140808",
  blood: "#140808",
};

export function drawSnap(snap) {
  if (!snap) return;
  S.mapW = snap.width;
  S.mapH = snap.height;
  renderSelectionChips(); // out-of-FOV dimming follows the live snapshot
  const d = syncViewSize();
  clampCamera();

  const x0 = S.cam.tx;
  const y0 = S.cam.ty;
  const visRows = snap.vision || [];
  // identity-first only: server always sends feat_ids + art catalog
  const feats = decodeFeatIds(snap.feat_ids);

  d.clear();
  for (let vy = 0; vy < S.viewRows; vy++) {
    const wy = y0 + vy;
    const visRow = visRows[wy] || "";
    for (let vx = 0; vx < S.viewCols; vx++) {
      const wx = x0 + vx;
      if (wy < 0 || wx < 0 || wy >= S.mapH || wx >= S.mapW) {
        d.draw(vx, vy, " ", S.theme.void, S.theme.void);
        continue;
      }
      const vch = visRow[wx] || " ";
      if (vch === " " || vch === "\0") {
        d.draw(vx, vy, " ", S.theme.void, S.theme.void);
        continue;
      }
      const fid = feats[wy * S.mapW + wx];
      const look = lookForFeat(fid);
      const ch = look.glyph || "?";
      let fg = materialColor(look.material, S.theme);
      let bg = MATERIAL_BG[look.material] ?? S.theme.void;
      if (vch === "m") {
        const f = S.theme.memoryFactor || 0.4;
        fg = dimColor(fg, f);
        bg = dimColor(bg, f + 0.05);
      }
      d.draw(vx, vy, ch, fg, bg);
    }
  }

  const ents = snap.entities || [];
  const SELECT_BG = S.theme.selection || "#003333";
  const trackedIds = new Set(
    S.tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  for (const ent of ents) {
    const vx = ent.x - x0;
    const vy = ent.y - y0;
    if (vx < 0 || vy < 0 || vx >= S.viewCols || vy >= S.viewRows) continue;
    if (ent.kind === "agent") {
      // Server already FOV-gates; tracked agents keep personal color.
      const look = lookForEntity(ent);
      const tr = S.tracked.find((t) => t.agent_id === ent.id);
      const fg = tr ? tr.color : materialColor(look.material, S.theme);
      const bg = S.selectedAgentIds.has(ent.id) ? SELECT_BG : S.theme.void;
      let glyph = look.glyph || "@";
      if (ent.name && /[A-Za-z]/.test(ent.name[0])) {
        glyph = ent.name[0].toUpperCase();
      }
      d.draw(vx, vy, glyph, fg, bg);
    } else {
      const look = lookForEntity(ent);
      const fg = materialColor(look.material, S.theme);
      d.draw(vx, vy, look.glyph || ent.glyph || "?", fg, S.theme.void);
    }
  }
  // tracked focus ring
  for (const t of S.tracked) {
    if (t.x == null || t.y == null) continue;
    const vx = t.x - x0;
    const vy = t.y - y0;
    if (vx < 0 || vy < 0 || vx >= S.viewCols || vy >= S.viewRows) continue;
    if (S.followToken === t.token) {
      d.draw(vx, vy, "@", t.color, "#1e1e1e");
    }
  }

  el.map.style.left = "0px";
  el.map.style.top = "0px";
  const lw = S.viewCols * cellSize();
  const lh = S.viewRows * cellSize();
  el.map.style.marginLeft =
    lw < el.viewport.clientWidth
      ? Math.floor((el.viewport.clientWidth - lw) / 2) + "px"
      : "0px";
  el.map.style.marginTop =
    lh < el.viewport.clientHeight
      ? Math.floor((el.viewport.clientHeight - lh) / 2) + "px"
      : "0px";
}

// ---------------------------------------------------------------- inspect

export function showInspectPopup(title, html) {
  if (!el.inspectPopup || !el.inspectTitle || !el.inspectBody) return;
  el.inspectTitle.textContent = title;
  el.inspectBody.innerHTML = html;
  el.inspectPopup.classList.add("visible");
}

export function hideInspectPopup() {
  if (!el.inspectPopup) return;
  el.inspectPopup.classList.remove("visible");
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

export function renderEntityInspect(e) {
  const kind = e.kind || "unknown";
  const glyph = e.glyph || "?";
  const title = `[${glyph}] ${kind}${e.name ? " — " + e.name : ""}`;
  const html =
    `<div class="muted">position (${e.x}, ${e.y}) · id ${e.id}</div>` +
    renderKV(e, ["id", "x", "y", "glyph"]);
  showInspectPopup(title, html);
}

export function renderCellInspect(c) {
  const title = `[${c.glyph || " "}] ${c.name || "cell"}`;
  const html =
    `<div class="muted">position (${c.x}, ${c.y}) · feat ${c.feat_id}</div>` +
    renderKV(c, ["x", "y", "glyph", "name", "feat_id"]);
  showInspectPopup(title, html);
}
