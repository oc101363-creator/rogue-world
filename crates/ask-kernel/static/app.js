/* ASK map viewer — large world + SC-style pan/zoom camera */

const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

const elViewport = document.getElementById("viewport");
const elMap = document.getElementById("map");
const elStatus = document.getElementById("status");
const elInfo = document.getElementById("info");
const elCam = document.getElementById("cam");

const TILE_FG = { "#": "#3d5c3d", ".": "#6a9a4a" };
const TILE_BG = { "#": "#0c140c", ".": "#121a10" };
const KIND_COLOR = {
  agent: "#ffe066",
  tree: "#2dd36f",
  iron: "#b0c4de",
  hut: "#ff9f43",
};

/** Base pixel size of one cell before camera zoom */
const BASE_TILE = 14;

let display = null;
let mapW = 0;
let mapH = 0;
let lastSnap = null;

// Camera: screen_px = world_px * scale + pan
const cam = {
  scale: 1,
  x: 0, // pan in screen px
  y: 0,
  follow: true,
  minScale: 0.15,
  maxScale: 4,
};

let dragging = false;
let dragLast = null;
let spaceDown = false;

function worldPixelSize() {
  return {
    w: mapW * BASE_TILE * cam.scale,
    h: mapH * BASE_TILE * cam.scale,
  };
}

function applyTransform() {
  elMap.style.transform = `translate(${cam.x}px, ${cam.y}px) scale(${cam.scale})`;
  elCam.textContent = `zoom ${(cam.scale * 100).toFixed(0)}%  ${
    cam.follow ? "FOLLOW" : "FREE"
  }`;
}

function clampPan() {
  const { w, h } = worldPixelSize();
  const vw = elViewport.clientWidth;
  const vh = elViewport.clientHeight;
  // Allow some overscroll margin
  const margin = 80;
  if (w + margin * 2 < vw) {
    cam.x = (vw - w) / 2;
  } else {
    cam.x = Math.min(margin, Math.max(vw - w - margin, cam.x));
  }
  if (h + margin * 2 < vh) {
    cam.y = (vh - h) / 2;
  } else {
    cam.y = Math.min(margin, Math.max(vh - h - margin, cam.y));
  }
}

function centerOnTile(tx, ty) {
  const vw = elViewport.clientWidth;
  const vh = elViewport.clientHeight;
  const px = (tx + 0.5) * BASE_TILE * cam.scale;
  const py = (ty + 0.5) * BASE_TILE * cam.scale;
  cam.x = vw / 2 - px;
  cam.y = vh / 2 - py;
  clampPan();
  applyTransform();
}

function zoomAt(screenX, screenY, factor) {
  const prev = cam.scale;
  let next = prev * factor;
  next = Math.max(cam.minScale, Math.min(cam.maxScale, next));
  if (next === prev) return;

  // Keep world point under cursor stable
  const wx = (screenX - cam.x) / prev;
  const wy = (screenY - cam.y) / prev;
  cam.scale = next;
  cam.x = screenX - wx * next;
  cam.y = screenY - wy * next;
  cam.follow = false;
  clampPan();
  applyTransform();
}

function ensureDisplay(w, h) {
  if (display && mapW === w && mapH === h) return display;
  elMap.innerHTML = "";
  display = new ROT.Display({
    width: w,
    height: h,
    fontSize: BASE_TILE,
    fontFamily: "ui-monospace, Menlo, monospace",
    bg: "#050805",
    fg: "#33ff66",
    forceSquareRatio: true,
    spacing: 1,
  });
  const canvas = display.getContainer();
  // Force canvas CSS size = logical tile grid (transform scales it)
  canvas.style.width = w * BASE_TILE + "px";
  canvas.style.height = h * BASE_TILE + "px";
  elMap.appendChild(canvas);
  elMap.style.width = w * BASE_TILE + "px";
  elMap.style.height = h * BASE_TILE + "px";
  mapW = w;
  mapH = h;

  // Initial fit: show ~40% of map width or whole map if smaller
  const fit = Math.min(
    elViewport.clientWidth / (w * BASE_TILE),
    elViewport.clientHeight / (h * BASE_TILE),
  );
  cam.scale = Math.max(cam.minScale, Math.min(1.2, fit * 1.15));
  cam.follow = true;
  applyTransform();
  return display;
}

function drawSnap(snap) {
  const d = ensureDisplay(snap.width, snap.height);
  d.clear();
  for (let y = 0; y < snap.height; y++) {
    const row = snap.tiles[y] || "";
    for (let x = 0; x < snap.width; x++) {
      const ch = row[x] || " ";
      d.draw(x, y, ch, TILE_FG[ch] || "#444", TILE_BG[ch] || "#050805");
    }
  }
  const ents = snap.entities || [];
  for (const e of ents.filter((x) => x.kind !== "agent")) {
    d.draw(e.x, e.y, e.glyph || "?", KIND_COLOR[e.kind] || "#fff", "#0a180a");
  }
  for (const e of ents.filter((x) => x.kind === "agent")) {
    d.draw(e.x, e.y, "@", KIND_COLOR.agent, "#1a280a");
  }
}

function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;
  lastSnap = snap;
  drawSnap(snap);

  const ents = snap.entities || [];
  const agent = ents.find((e) => e.kind === "agent");
  const trees = ents.filter((e) => e.kind === "tree").length;
  const irons = ents.filter((e) => e.kind === "iron").length;
  const huts = ents.filter((e) => e.kind === "hut").length;
  elInfo.textContent = agent
    ? `t=${snap.tick}  ${snap.width}×${snap.height}  @(${agent.x},${agent.y})  wood=${agent.wood ?? 0} iron=${agent.iron ?? 0}  T=${trees} I=${irons} H=${huts}`
    : `t=${snap.tick}  ${snap.width}×${snap.height}`;

  if (cam.follow && agent) {
    centerOnTile(agent.x, agent.y);
  } else {
    clampPan();
    applyTransform();
  }
}

// --- input ---
elViewport.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const rect = elViewport.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    zoomAt(sx, sy, factor);
  },
  { passive: false },
);

elViewport.addEventListener("mousedown", (e) => {
  if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;
  dragging = true;
  dragLast = { x: e.clientX, y: e.clientY };
  elViewport.classList.add("dragging");
  cam.follow = false;
  e.preventDefault();
});

window.addEventListener("mousemove", (e) => {
  if (!dragging || !dragLast) return;
  const dx = e.clientX - dragLast.x;
  const dy = e.clientY - dragLast.y;
  dragLast = { x: e.clientX, y: e.clientY };
  cam.x += dx;
  cam.y += dy;
  clampPan();
  applyTransform();
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
    if (!spaceDown) {
      spaceDown = true;
      cam.follow = true;
      if (lastSnap) {
        const agent = (lastSnap.entities || []).find((x) => x.kind === "agent");
        if (agent) centerOnTile(agent.x, agent.y);
      }
    }
  }
  if (e.key === "+" || e.key === "=") {
    zoomAt(elViewport.clientWidth / 2, elViewport.clientHeight / 2, 1.15);
  }
  if (e.key === "-" || e.key === "_") {
    zoomAt(elViewport.clientWidth / 2, elViewport.clientHeight / 2, 1 / 1.15);
  }
  if (e.key === "r" || e.key === "R") {
    cam.follow = true;
    cam.scale = Math.min(
      1.2,
      Math.min(
        elViewport.clientWidth / (mapW * BASE_TILE || 1),
        elViewport.clientHeight / (mapH * BASE_TILE || 1),
      ) * 1.1,
    );
    if (lastSnap) {
      const agent = (lastSnap.entities || []).find((x) => x.kind === "agent");
      if (agent) centerOnTile(agent.x, agent.y);
    }
  }
  // arrow pan when not following
  const step = 40;
  if (!cam.follow) {
    if (e.key === "ArrowLeft") {
      cam.x += step;
      clampPan();
      applyTransform();
    }
    if (e.key === "ArrowRight") {
      cam.x -= step;
      clampPan();
      applyTransform();
    }
    if (e.key === "ArrowUp") {
      cam.y += step;
      clampPan();
      applyTransform();
    }
    if (e.key === "ArrowDown") {
      cam.y -= step;
      clampPan();
      applyTransform();
    }
  }
});

window.addEventListener("keyup", (e) => {
  if (e.code === "Space") spaceDown = false;
});

window.addEventListener("resize", () => {
  clampPan();
  applyTransform();
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

connect();
