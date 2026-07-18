/* ASK viewer — map: ROT display, camera, draw, coordinate queries.
 * Owns #map canvas + #select-box overlay. Emits camera-changed. */

import { el, S, ZOOM_STEPS } from "./state.js";
import { on, emit } from "./bus.js";
import { decodeFeatIds, lookForFeat, lookForEntity, materialColor } from "./art.js";

/** Install the ROT display into the #map grid cell. */
export function mountMapview(root) {
  S.mapRoot = root; // syncViewSize appends the canvas here

  // the draw pipeline: net.js emits "snapshot", the map draws itself
  on("snapshot", (snap) => {
    S.mapW = snap.width;
    S.mapH = snap.height;
    syncViewSize();
    if (S.cam.follow) {
      const a = focusAgent();
      if (a) centerOnTile(a.x, a.y);
    }
    drawSnap(snap);
  });

  // selection ring follows the shared selection set
  on("selection-changed", () => {
    if (S.lastSnap) drawSnap(S.lastSnap);
  });
}

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
  S.viewCols = Math.max(8, Math.ceil(document.getElementById("viewport").clientWidth / cs) + 1);
  S.viewRows = Math.max(8, Math.ceil(document.getElementById("viewport").clientHeight / cs) + 1);
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
    S.mapRoot.innerHTML = "";
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
    S.mapRoot.appendChild(canvas);
    el.map.style.width = lw + "px";
    el.map.style.height = lh + "px";
    S.display._viewCols = S.viewCols;
    S.display._viewRows = S.viewRows;
    S.display._cellSize = cs;
    S.display._dpr = dpr;
    S.display._themeId = S.theme.id;
  }
  clampCamera();
  return S.display;
}

export function clampCamera() {
  if (S.mapW <= 0 || S.mapH <= 0) return;
  S.cam.tx = Math.max(0, Math.min(S.mapW - S.viewCols, Math.floor(S.cam.tx)));
  S.cam.ty = Math.max(0, Math.min(S.mapH - S.viewRows, Math.floor(S.cam.ty)));
  emit("camera-changed");
}

export function centerOnTile(tx, ty) {
  S.cam.tx = tx - Math.floor(S.viewCols / 2);
  S.cam.ty = ty - Math.floor(S.viewRows / 2);
  clampCamera();
  emit("camera-changed");
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
  emit("camera-changed");
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
  // selection chips re-render via the dispatch panel on `snapshot` (Task 6), not here
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
