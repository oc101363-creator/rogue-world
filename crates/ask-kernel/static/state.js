/* ASK viewer — shared state (single state bag) + storage.
 * No imports: every other module may read/write S.* freely.
 * Element refs live in the modules that own them (getElementById). */

import { getTheme } from "./themes.js";
import { emit } from "./bus.js";

export const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

export const ZOOM_STEPS = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40];
export const THEME_KEY = "ask-theme";
export const TRACK_KEY = "ask-track-tokens";
export const PRESETS_KEY = "ask-presets-v1";
export const SQUADS_KEY = "ask-squads-v1";
export const TRACK_COLORS = ["#ffff00", "#00ffff", "#00ff00", "#ff00ff", "#ff8800", "#88ff88"];

export function loadTracked() {
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

export function saveTracked() {
  localStorage.setItem(
    TRACK_KEY,
    JSON.stringify(
      S.tracked.map((t) => ({
        token: t.token,
        agent_id: t.agent_id,
        name: t.name,
        purpose: t.purpose,
        color: t.color,
      })),
    ),
  );
}

export function loadPresets() {
  try {
    const raw = JSON.parse(localStorage.getItem(PRESETS_KEY) || "[]");
    return Array.isArray(raw) ? raw : [];
  } catch (_) {
    return [];
  }
}

export function savePresets(presets) {
  localStorage.setItem(PRESETS_KEY, JSON.stringify(presets));
}

/** Named selection sets ("miners", "team A") — operator-side, local only. */
export function loadSquads() {
  try {
    const raw = JSON.parse(localStorage.getItem(SQUADS_KEY) || "[]");
    return Array.isArray(raw) ? raw : [];
  } catch (_) {
    return [];
  }
}

export function saveSquads(squads) {
  localStorage.setItem(SQUADS_KEY, JSON.stringify(squads));
}

/** Display name for an agent id: snapshot name > tracked name > #id. */
export function agentName(id) {
  const en = (S.lastSnap && S.lastSnap.entities || []).find((e) => e.id === id);
  if (en && en.name) return en.name;
  const t = S.tracked.find((t) => t.agent_id === id);
  if (t && t.name) return t.name;
  return `#${id}`;
}

/** The mutable viewer state. One bag, no hidden globals. */
export const S = {
  display: null,
  mapRoot: null,
  mapW: 0,
  mapH: 0,
  lastSnap: null,
  viewCols: 0,
  viewRows: 0,
  theme: getTheme(localStorage.getItem(THEME_KEY) || "rogue-80"),
  ws: null,
  humanControl: false,
  lastMe: null,
  /** pending direction for o/c/f/t commands */
  pendingDirCmd: null, // "open" | "close" | "attack" | "dig" | "place" | "scoop" | null
  /** @type {{token:string, agent_id?:number, name?:string, purpose?:string, x?:number, y?:number, color:string}[]} */
  tracked: loadTracked(),
  /** which tracked agent the camera follows */
  followToken: null,
  selecting: false,
  selectStart: null, // { sx, sy, wx, wy }
  selectedAgentIds: new Set(),
  /** last broadcast's per-target receipts [{agent, msg_id, ok, reason, read_tick}] */
  delivery: [],
  cam: { tx: 0, ty: 0, zi: 4, follow: true },
};
S.followToken = S.tracked.length ? S.tracked[0].token : null;

/** Selection is shared S state; every mutation emits for chips+highlight. */
export function setSelectedAgents(ids) {
  S.selectedAgentIds = new Set(ids);
  emit("selection-changed");
}
export function addSelectedAgents(ids) {
  for (const id of ids) S.selectedAgentIds.add(id);
  emit("selection-changed");
}
export function toggleSelectAgent(id) {
  if (S.selectedAgentIds.has(id)) S.selectedAgentIds.delete(id);
  else S.selectedAgentIds.add(id);
  emit("selection-changed");
}

/** Token used for inspect/message calls: focus, else first tracked. */
export function inspectToken() {
  return S.followToken || (S.tracked[0] && S.tracked[0].token);
}
