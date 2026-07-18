/* ASK viewer — rendering: panels, inspect popup.
 * Map/camera/draw moved to mapview.js (T4); hud/tracker/theme moved to
 * panels/ (T5). Imports state only. */

import { el, S, loadPresets, agentName } from "./state.js";
import { visibleAgentIds, updateSelectionHighlight } from "./mapview.js";

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
