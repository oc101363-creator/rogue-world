/* ASK viewer — shared state (single state bag) + element refs + storage.
 * No imports: every other module may read/write S.* freely. */

export const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

export const el = {
  viewport: document.getElementById("viewport"),
  map: document.getElementById("map"),
  status: document.getElementById("status"),
  info: document.getElementById("info"),
  cam: document.getElementById("cam"),
  theme: document.getElementById("theme"),
  mode: document.getElementById("mode"),
  log: document.getElementById("log"),
  tokenInput: document.getElementById("token-input"),
  tokenAdd: document.getElementById("token-add"),
  tokenClear: document.getElementById("token-clear"),
  trackerList: document.getElementById("tracker-list"),
  trackerHint: document.getElementById("tracker-hint"),
  btnFollow: document.getElementById("btn-follow"),
  btnMock: document.getElementById("btn-mock"),
  inspectPopup: document.getElementById("inspect-popup"),
  inspectTitle: document.getElementById("inspect-title"),
  inspectBody: document.getElementById("inspect-body"),
  inspectClose: document.getElementById("inspect-close"),
  selectBox: document.getElementById("select-box"),
  selCount: document.getElementById("sel-count"),
  selAllVis: document.getElementById("sel-all-vis"),
  selClear: document.getElementById("sel-clear"),
  selPreset: document.getElementById("sel-preset"),
  selPresetDel: document.getElementById("sel-preset-del"),
  selPresetSave: document.getElementById("sel-preset-save"),
  selPresetName: document.getElementById("sel-preset-name"),
  selText: document.getElementById("sel-text"),
  selSend: document.getElementById("sel-send"),
};

export const ZOOM_STEPS = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40];
export const THEME_KEY = "ask-theme";
export const TRACK_KEY = "ask-track-tokens";
export const PRESETS_KEY = "ask-presets-v1";
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

/** The mutable viewer state. One bag, no hidden globals. */
export const S = {
  display: null,
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
  cam: { tx: 0, ty: 0, zi: 4, follow: true },
};
S.followToken = S.tracked.length ? S.tracked[0].token : null;

/** Token used for inspect/message calls: focus, else first tracked. */
export function inspectToken() {
  return S.followToken || (S.tracked[0] && S.tracked[0].token);
}
