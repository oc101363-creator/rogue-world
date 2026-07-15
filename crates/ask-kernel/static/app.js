/* ASK map viewer — pan/zoom + selectable terminal themes */

const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

const elViewport = document.getElementById("viewport");
const elMap = document.getElementById("map");
const elStatus = document.getElementById("status");
const elInfo = document.getElementById("info");
const elCam = document.getElementById("cam");
const elTheme = document.getElementById("theme");

const ZOOM_STEPS = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40];
const THEME_KEY = "ask-theme";

let display = null;
let mapW = 0;
let mapH = 0;
let lastSnap = null;
let viewCols = 0;
let viewRows = 0;
let theme = getTheme(localStorage.getItem(THEME_KEY) || "qud-viridian");

const cam = {
  tx: 0,
  ty: 0,
  zi: 4,
  follow: true,
};

function cellSize() {
  return ZOOM_STEPS[cam.zi];
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
    // force display recreate with new void bg
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

function drawSnap(snap) {
  if (!snap) return;
  mapW = snap.width;
  mapH = snap.height;
  const d = syncViewSize();
  clampCamera();

  const x0 = cam.tx;
  const y0 = cam.ty;
  const colorRows = snap.tile_colors || [];

  d.clear();
  for (let vy = 0; vy < viewRows; vy++) {
    const wy = y0 + vy;
    const row = snap.tiles[wy] || "";
    const colorRow = colorRows[wy] || "";
    for (let vx = 0; vx < viewCols; vx++) {
      const wx = x0 + vx;
      if (wy < 0 || wx < 0 || wy >= mapH || wx >= mapW) {
        d.draw(vx, vy, " ", theme.void, theme.void);
        continue;
      }
      const ch = row[wx] || " ";
      const letter = colorRow[wx] || "w";
      const fg = letterFg(letter);
      const bg = theme.cellBg(letter, ch);
      d.draw(vx, vy, ch, fg, bg);
    }
  }

  const ents = snap.entities || [];
  const e = theme.entities;
  for (const ent of ents) {
    const vx = ent.x - x0;
    const vy = ent.y - y0;
    if (vx < 0 || vy < 0 || vx >= viewCols || vy >= viewRows) continue;
    if (ent.kind === "agent") {
      d.draw(vx, vy, "@", e.agent, e.entityBg);
    } else {
      const fg = e[ent.kind] || e.iron;
      d.draw(vx, vy, ent.glyph || "?", fg, e.entityBg);
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

function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;
  lastSnap = snap;

  const ents = snap.entities || [];
  const agent = ents.find((e) => e.kind === "agent");
  const trees = ents.filter((e) => e.kind === "tree").length;
  const irons = ents.filter((e) => e.kind === "iron").length;
  const huts = ents.filter((e) => e.kind === "hut").length;
  elInfo.textContent = agent
    ? `t=${snap.tick}  ${snap.width}×${snap.height}  @(${agent.x},${agent.y})  wood=${agent.wood ?? 0} iron=${agent.iron ?? 0}  T=${trees} I=${irons} H=${huts}`
    : `t=${snap.tick}  ${snap.width}×${snap.height}`;

  if (cam.follow && agent) {
    mapW = snap.width;
    mapH = snap.height;
    syncViewSize();
    centerOnTile(agent.x, agent.y);
  }
  drawSnap(snap);
}

let dragging = false;
let dragLast = null;
let accumX = 0;
let accumY = 0;

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
  dragging = true;
  dragLast = { x: e.clientX, y: e.clientY };
  accumX = 0;
  accumY = 0;
  elViewport.classList.add("dragging");
  cam.follow = false;
  e.preventDefault();
});

window.addEventListener("mousemove", (e) => {
  if (!dragging || !dragLast) return;
  const dx = e.clientX - dragLast.x;
  const dy = e.clientY - dragLast.y;
  dragLast = { x: e.clientX, y: e.clientY };
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

window.addEventListener("mouseup", () => {
  dragging = false;
  dragLast = null;
  elViewport.classList.remove("dragging");
});

elViewport.addEventListener("contextmenu", (e) => e.preventDefault());

window.addEventListener("keydown", (e) => {
  if (e.code === "Space") {
    e.preventDefault();
    cam.follow = true;
    if (lastSnap) {
      const agent = (lastSnap.entities || []).find((x) => x.kind === "agent");
      if (agent) {
        centerOnTile(agent.x, agent.y);
        drawSnap(lastSnap);
      }
    }
  }
  if (e.key === "+" || e.key === "=") {
    zoomBy(1, elViewport.clientWidth / 2, elViewport.clientHeight / 2);
  }
  if (e.key === "-" || e.key === "_") {
    zoomBy(-1, elViewport.clientWidth / 2, elViewport.clientHeight / 2);
  }
  if (e.key === "r" || e.key === "R") {
    cam.zi = 4;
    cam.follow = true;
    syncViewSize();
    if (lastSnap) {
      const agent = (lastSnap.entities || []).find((x) => x.kind === "agent");
      if (agent) centerOnTile(agent.x, agent.y);
      drawSnap(lastSnap);
    }
  }
  if (!cam.follow) {
    let moved = false;
    if (e.key === "ArrowLeft") {
      cam.tx -= 2;
      moved = true;
    }
    if (e.key === "ArrowRight") {
      cam.tx += 2;
      moved = true;
    }
    if (e.key === "ArrowUp") {
      cam.ty -= 2;
      moved = true;
    }
    if (e.key === "ArrowDown") {
      cam.ty += 2;
      moved = true;
    }
    if (moved) {
      clampCamera();
      if (lastSnap) drawSnap(lastSnap);
      updateHudCam();
    }
  }
});

window.addEventListener("resize", () => {
  if (lastSnap) {
    if (cam.follow) {
      const agent = (lastSnap.entities || []).find((x) => x.kind === "agent");
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
  const ws = new WebSocket(WS_URL);
  ws.onopen = () => {
    elStatus.textContent = "live";
    elStatus.className = "online";
  };
  ws.onclose = () => {
    elStatus.textContent = "offline";
    elStatus.className = "offline";
    setTimeout(connect, 1200);
  };
  ws.onerror = () => ws.close();
  ws.onmessage = (ev) => {
    try {
      applySnapshot(JSON.parse(ev.data));
    } catch (_) {}
  };
}

setupThemeSelect();
connect();
