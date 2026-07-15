/* ASK Viewer — connects to kernel WebSocket and draws with rot.js */

const WS_URL =
  (location.protocol === "https:" ? "wss://" : "ws://") +
  location.host +
  "/ws";

const el = {
  status: document.getElementById("status"),
  map: document.getElementById("map"),
  ascii: document.getElementById("ascii"),
  tick: document.getElementById("tick"),
  size: document.getElementById("size"),
  agent: document.getElementById("agent"),
  inv: document.getElementById("inv"),
  entities: document.getElementById("entities"),
  events: document.getElementById("events"),
};

let display = null;
let lastW = 0;
let lastH = 0;

const TILE_FG = { "#": "#5a7a5a", ".": "#1a3a1a" };
const TILE_BG = { "#": "#0a120a", ".": "#050805" };
const KIND_COLOR = {
  agent: "#33ff66",
  tree: "#2ecc71",
  iron: "#c0c0c0",
  hut: "#ffb000",
};

function setStatus(online, text) {
  el.status.textContent = text;
  el.status.className = "pill " + (online ? "online" : "offline");
}

function ensureDisplay(w, h) {
  if (display && lastW === w && lastH === h) return display;
  el.map.innerHTML = "";
  display = new ROT.Display({
    width: w,
    height: h,
    fontSize: 20,
    fontFamily: "ui-monospace, Menlo, monospace",
    bg: "#050805",
    fg: "#33ff66",
    forceSquareRatio: true,
  });
  el.map.appendChild(display.getContainer());
  lastW = w;
  lastH = h;
  return display;
}

function eventLabel(e) {
  if (!e || !e.type) return JSON.stringify(e);
  const t = e.type;
  if (t === "moved") return `MOVED ${e.entity} (${e.from})→(${e.to})`;
  if (t === "harvested") return `HARVEST ${e.kind} +${e.amount}`;
  if (t === "built") return `BUILT hut @ (${e.at})`;
  if (t === "resource_depleted") return `DEPLETED ${e.entity}`;
  if (t === "tick_started") return `TICK ${e.tick}`;
  return t.toUpperCase();
}

function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;

  el.tick.textContent = String(snap.tick);
  el.size.textContent = `${snap.width}×${snap.height}`;
  el.ascii.textContent = snap.map || "";

  const agents = (snap.entities || []).filter((e) => e.kind === "agent");
  if (agents[0]) {
    el.agent.textContent = `#${agents[0].id} @ (${agents[0].x},${agents[0].y})`;
    el.inv.textContent = `${agents[0].wood ?? 0} / ${agents[0].iron ?? 0}`;
  } else {
    el.agent.textContent = "—";
    el.inv.textContent = "—";
  }

  el.entities.innerHTML = "";
  for (const e of snap.entities || []) {
    const li = document.createElement("li");
    let extra = "";
    if (e.amount != null) extra = ` amt=${e.amount}`;
    if (e.wood != null) extra = ` w=${e.wood} i=${e.iron}`;
    li.textContent = `${e.glyph} ${e.kind}#${e.id} (${e.x},${e.y})${extra}`;
    el.entities.appendChild(li);
  }

  el.events.innerHTML = "";
  const events = (snap.recent_events || []).slice().reverse().slice(0, 24);
  for (const e of events) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>${e.type || "?"}</strong> ${eventLabel(e)}`;
    el.events.appendChild(li);
  }

  const d = ensureDisplay(snap.width, snap.height);
  d.clear();
  for (let y = 0; y < snap.height; y++) {
    const row = snap.tiles[y] || "";
    for (let x = 0; x < snap.width; x++) {
      const ch = row[x] || " ";
      d.draw(x, y, ch, TILE_FG[ch] || "#2a4a2a", TILE_BG[ch] || "#050805");
    }
  }
  // non-agents then agents
  const ents = snap.entities || [];
  for (const e of ents.filter((x) => x.kind !== "agent")) {
    d.draw(e.x, e.y, e.glyph || "?", KIND_COLOR[e.kind] || "#fff", "#0a2010");
  }
  for (const e of ents.filter((x) => x.kind === "agent")) {
    d.draw(e.x, e.y, e.glyph || "A", KIND_COLOR.agent, "#0a2010");
  }
}

function connect() {
  setStatus(false, "connecting…");
  const ws = new WebSocket(WS_URL);
  ws.onopen = () => setStatus(true, "online");
  ws.onclose = () => {
    setStatus(false, "offline — retry");
    setTimeout(connect, 1500);
  };
  ws.onerror = () => ws.close();
  ws.onmessage = (ev) => {
    try {
      applySnapshot(JSON.parse(ev.data));
    } catch (e) {
      console.warn("bad snapshot", e);
    }
  };
}

connect();
