/* Map-only ASK viewer */

const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws";

const elMap = document.getElementById("map");
const elStatus = document.getElementById("status");
const elInfo = document.getElementById("info");

let display = null;
let lastW = 0;
let lastH = 0;

const TILE_FG = { "#": "#3d5c3d", ".": "#6a9a4a" };
const TILE_BG = { "#": "#0c140c", ".": "#121a10" };
const KIND_COLOR = {
  agent: "#ffe066",
  tree: "#2dd36f",
  iron: "#b0c4de",
  hut: "#ff9f43",
};

function ensureDisplay(w, h) {
  // Fit font size to window so large maps still fill the view
  const pad = 16;
  const maxFw = Math.floor((window.innerWidth - pad) / w);
  const maxFh = Math.floor((window.innerHeight - pad) / h);
  const fontSize = Math.max(8, Math.min(22, maxFw, maxFh));

  if (display && lastW === w && lastH === h && display._fontSize === fontSize) {
    return display;
  }
  elMap.innerHTML = "";
  display = new ROT.Display({
    width: w,
    height: h,
    fontSize,
    fontFamily: "ui-monospace, Menlo, monospace",
    bg: "#050805",
    fg: "#33ff66",
    forceSquareRatio: true,
  });
  display._fontSize = fontSize;
  elMap.appendChild(display.getContainer());
  lastW = w;
  lastH = h;
  return display;
}

function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;
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

  const agent = ents.find((e) => e.kind === "agent");
  const trees = ents.filter((e) => e.kind === "tree").length;
  const irons = ents.filter((e) => e.kind === "iron").length;
  const huts = ents.filter((e) => e.kind === "hut").length;
  elInfo.textContent = agent
    ? `t=${snap.tick}  ${snap.width}×${snap.height}  @(${agent.x},${agent.y})  wood=${agent.wood ?? 0} iron=${agent.iron ?? 0}  T=${trees} I=${irons} H=${huts}`
    : `t=${snap.tick}`;
}

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

window.addEventListener("resize", () => {
  lastW = 0; // force redraw on next snap
});

connect();
