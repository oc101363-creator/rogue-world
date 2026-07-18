/* ASK viewer — server comms: WS lifecycle, snapshot application, actions,
 * tracking, messages, inspect fetches. Imports state + render. */

import { el, S, WS_URL, TRACK_COLORS, inspectToken, saveTracked } from "./state.js";
import { ensureArtCatalog } from "./art.js";
import {
  pushLog,
  formatEvents,
  renderTracker,
  updateModeHud,
  drawSnap,
  syncViewSize,
  centerOnTile,
  focusAgent,
  renderEntityInspect,
  renderCellInspect,
  renderDelivery,
} from "./render.js";

// ---------------------------------------------------------------- tracking

export async function refreshTracked() {
  if (!S.tracked.length) {
    renderTracker();
    return;
  }
  await Promise.all(
    S.tracked.map(async (t) => {
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
  if (S.cam.follow && S.followToken) {
    const t = S.tracked.find((x) => x.token === S.followToken);
    if (t && t.x != null) {
      centerOnTile(t.x, t.y);
      if (S.lastSnap) drawSnap(S.lastSnap);
    }
  }
}

export async function addToken(tok) {
  const token = (tok || "").trim();
  if (!token) return;
  if (S.tracked.some((t) => t.token === token)) {
    pushLog("already tracked");
    return;
  }
  const color = TRACK_COLORS[S.tracked.length % TRACK_COLORS.length];
  S.tracked.push({ token, color });
  saveTracked();
  await refreshTracked();
  S.followToken = token;
  S.cam.follow = true;
  sendSubscribe();
  pushLog(`+TRACK ${token.slice(0, 16)}…`);
}


export function clearTracked() {
  S.tracked = [];
  S.followToken = null;
  S.lastMe = null;
  saveTracked();
  renderTracker();
  pushLog("CLEARED tracked tokens");
  if (S.lastSnap) {
    drawSnap(S.lastSnap);
  }
  sendSubscribe();
}

export async function refreshMe() {
  const token = inspectToken();
  if (!token) return;
  try {
    const r = await fetch("/api/view?token=" + encodeURIComponent(token));
    if (!r.ok) return;
    const d = await r.json();
    if (!d.ok || !d.self) return;
    S.lastMe = d;
    const t = S.tracked.find((x) => x.token === token);
    if (t) {
      t.agent_id = d.self.id;
      if (d.self.name) t.name = d.self.name;
      t.x = d.self.x;
      t.y = d.self.y;
    }
    saveTracked();
    renderTracker();
  } catch (_) {}
}

// ---------------------------------------------------------------- websocket

export function sendSubscribe() {
  if (!S.ws || S.ws.readyState !== WebSocket.OPEN) return;
  const focus = inspectToken() || null;
  S.ws.send(
    JSON.stringify({
      type: "subscribe",
      tokens: S.tracked.map((t) => t.token),
      focus,
    }),
  );
}

export function applySnapshot(snap) {
  if (!snap || snap.type !== "snapshot") return;
  const prevTick = S.lastSnap ? S.lastSnap.tick : -1;
  S.lastSnap = snap;
  if (snap.catalog_version != null) {
    ensureArtCatalog(snap.catalog_version).catch(function () {});
  }

  const ents = snap.entities || [];
  const trackedIds = new Set(
    S.tracked.map((t) => t.agent_id).filter((id) => id != null),
  );
  const agents = ents.filter((e) => e.kind === "agent" && trackedIds.has(e.id));
  // sync tracked poses from snapshot when agent_id known
  for (const t of S.tracked) {
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
  const huts = ents.filter((e) => e.kind === "hut").length;
  const mons = ents.filter((e) => e.kind === "monster").length;
  const hp = agent && agent.hp != null ? ` hp=${agent.hp}/${agent.max_hp ?? "?"}` : "";
  const pack =
    agent && agent.items && agent.items.length ? ` pack=${agent.items.length}` : "";
  const interactionsSrc =
    S.lastMe && S.lastMe.can && S.lastMe.can.interactions
      ? S.lastMe.can.interactions
      : snap.interactions || [];
  const acts = interactionsSrc
    .slice(0, 3)
    .map((i) => i.verb)
    .join(",");
  const actHud = acts ? ` [${acts}]` : "";
  const names = agents
    .map((a) => a.name || `@${a.id}`)
    .slice(0, 4)
    .join(",");
  el.info.textContent = `t=${snap.tick} ${snap.width}x${snap.height} agents=${agents.length}${
    agent ? ` focus=${agent.name || agent.id} @(${agent.x},${agent.y})${hp}${pack}` : ""
  } T=${trees} H=${huts} M=${mons}${actHud}${names ? " {" + names + "}" : ""}`;

  if (snap.tick !== prevTick && snap.recent_events) {
    formatEvents(snap.recent_events.filter((e) => e.type !== "tick_started"));
  }
  if (snap.tick !== prevTick && snap.tick % 5 === 0) {
    refreshTracked();
    refreshMe();
  }

  if (S.cam.follow) {
    S.mapW = snap.width;
    S.mapH = snap.height;
    syncViewSize();
    if (agent) centerOnTile(agent.x, agent.y);
  }
  drawSnap(snap);
}

export function connect() {
  el.status.textContent = "connecting";
  el.status.className = "offline";
  ensureArtCatalog().catch(function () {
    pushLog("ART: catalog load failed");
  });
  S.ws = new WebSocket(WS_URL);
  S.ws.onopen = () => {
    el.status.textContent = "live";
    el.status.className = "online";
    sendSubscribe();
  };
  S.ws.onclose = () => {
    el.status.textContent = "offline";
    el.status.className = "offline";
    S.ws = null;
    setTimeout(connect, 1200);
  };
  S.ws.onerror = () => S.ws && S.ws.close();
  S.ws.onmessage = (ev) => {
    try {
      applySnapshot(JSON.parse(ev.data));
    } catch (_) {}
  };
}


// ---------------------------------------------------------------- actions

/** Send action to kernel (WS preferred, HTTP fallback). */
export function sendAction(action) {
  const token = inspectToken();
  if (!token) {
    pushLog("NO TRACKED AGENT — paste a token");
    return;
  }
  const agent = focusAgent();
  const body = {
    type: "action",
    token,
    agent_id: agent ? agent.id : undefined,
    tick: S.lastSnap ? S.lastSnap.tick : undefined,
    action,
  };
  S.humanControl = true;
  updateModeHud();
  if (S.ws && S.ws.readyState === WebSocket.OPEN) {
    S.ws.send(JSON.stringify(body));
    return;
  }
  fetch("/api/act", {
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

export function setHumanControl(on) {
  S.humanControl = on;
  updateModeHud();
  // /api/control is operator-only: the server requires the dev token.
  // Paste the dev token into the TRACK panel to enable this switch.
  const token = inspectToken();
  const msg = { type: "control", human_control: on, token };
  if (S.ws && S.ws.readyState === WebSocket.OPEN) {
    S.ws.send(JSON.stringify(msg));
  } else {
    fetch("/api/control", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ human_control: on, token }),
    }).catch(() => {});
  }
}

export async function sendPromptToSelected(text) {
  const token = inspectToken();
  if (!token) {
    pushLog("SEND: track a token first");
    return;
  }
  if (!S.selectedAgentIds.size) {
    pushLog("SEND: select agents first");
    return;
  }
  if (!text.trim()) {
    pushLog("SEND: empty prompt");
    return;
  }
  if (text.length > 500) {
    pushLog("SEND: prompt too long (>500 UTF-16 code units)");
    return;
  }
  const targets = Array.from(S.selectedAgentIds);
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
    // per-target receipts → delivery rows, then poll for read stamps
    S.delivery = (d.results || []).map((x) => ({
      agent: x.id,
      msg_id: x.msg_id,
      ok: !!x.ok,
      reason: x.reason,
      read_tick: null,
    }));
    renderDelivery();
    pushLog(`SEND → ${d.sent} agents, ${d.rejected} rejected`);
    pollMessageStatus();
  } catch (_) {
    pushLog("SEND: network");
  }
}

/** Short-window poll: fill in read receipts, stop when all read or ~30s. */
async function pollMessageStatus() {
  const pending = () => S.delivery.filter((d) => d.ok && d.read_tick == null);
  for (let i = 0; i < 15 && pending().length; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    const token = inspectToken();
    if (!token) return;
    const ids = pending().map((d) => d.msg_id).join(",");
    try {
      const r = await fetch(
        `/api/message/status?token=${encodeURIComponent(token)}&ids=${ids}`,
      );
      const d = await r.json();
      if (!d.ok) return;
      for (const s of d.statuses || []) {
        const row = S.delivery.find((x) => x.msg_id === s.id);
        if (row) row.read_tick = s.read_tick;
      }
      renderDelivery();
    } catch (_) {
      return;
    }
  }
}

/** Operator inbox (dev token): messages agents sent to target 0. */
export async function fetchOperatorInbox() {
  const token = inspectToken();
  if (!token) {
    pushLog("INBOX: track a token first");
    return;
  }
  try {
    const r = await fetch("/api/message/inbox?token=" + encodeURIComponent(token));
    const d = await r.json();
    if (!d.ok) {
      pushLog("INBOX: " + (d.reason || "failed"));
      return;
    }
    if (!d.messages.length) {
      pushLog("INBOX: empty");
      return;
    }
    for (const m of d.messages) {
      pushLog(`◀ ${m.from} (tick ${m.sent_tick}): ${m.text}`);
    }
  } catch (_) {
    pushLog("INBOX: network");
  }
}

// ---------------------------------------------------------------- inspect fetches

export async function fetchEntityInspect(id) {
  const token = inspectToken();
  if (!token) {
    pushLog("INSPECT: track a token first");
    return;
  }
  try {
    const r = await fetch(`/api/entity?id=${id}&token=` + encodeURIComponent(token));
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

export async function fetchCellInspect(wx, wy) {
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
