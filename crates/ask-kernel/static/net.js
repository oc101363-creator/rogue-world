/* ASK viewer — server comms: WS lifecycle, snapshot application, actions,
 * tracking, messages, inspect fetches. Wire layer only: emits on the bus,
 * never touches the DOM or draw pipeline directly. */

import { S, WS_URL, TRACK_COLORS, inspectToken, saveTracked } from "./state.js";
import { ensureArtCatalog } from "./art.js";
import { focusAgent } from "./mapview.js";
import { on, emit, log } from "./bus.js";

// tracker re-renders on this event; net resubscribes with the new focus
on("tracked-changed", () => sendSubscribe());

// ---------------------------------------------------------------- tracking

export async function refreshTracked() {
  if (!S.tracked.length) {
    emit("tracked-changed");
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
  emit("tracked-changed");
  if (S.cam.follow && S.followToken) {
    const t = S.tracked.find((x) => x.token === S.followToken);
    if (t && t.x != null && S.lastSnap) {
      emit("snapshot", S.lastSnap); // re-centers on focus + redraws
    }
  }
}

export async function addToken(tok) {
  const token = (tok || "").trim();
  if (!token) return;
  if (S.tracked.some((t) => t.token === token)) {
    log("already tracked");
    return;
  }
  const color = TRACK_COLORS[S.tracked.length % TRACK_COLORS.length];
  S.tracked.push({ token, color });
  saveTracked();
  await refreshTracked();
  S.followToken = token;
  S.cam.follow = true;
  emit("tracked-changed"); // tracker re-renders, net resubscribes
  log(`+TRACK ${token.slice(0, 16)}…`);
}


export function clearTracked() {
  S.tracked = [];
  S.followToken = null;
  S.lastMe = null;
  saveTracked();
  log("CLEARED tracked tokens");
  if (S.lastSnap) {
    emit("snapshot", S.lastSnap); // redraw without the old focus ring
  }
  emit("tracked-changed"); // tracker re-renders, net resubscribes
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
    emit("tracked-changed");
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
  emit(
    "hud-info",
    `t=${snap.tick} ${snap.width}x${snap.height} agents=${agents.length}${
      agent ? ` focus=${agent.name || agent.id} @(${agent.x},${agent.y})${hp}${pack}` : ""
    } T=${trees} H=${huts} M=${mons}${actHud}${names ? " {" + names + "}" : ""}`,
  );

  if (snap.tick !== prevTick && snap.recent_events) {
    emit("events", snap.recent_events.filter((e) => e.type !== "tick_started"));
  }
  if (snap.tick !== prevTick && snap.tick % 5 === 0) {
    refreshTracked();
    refreshMe();
  }

  emit("snapshot", snap);
}

export function connect() {
  emit("conn-status", { text: "connecting", online: false });
  ensureArtCatalog().catch(function () {
    log("ART: catalog load failed");
  });
  S.ws = new WebSocket(WS_URL);
  S.ws.onopen = () => {
    emit("conn-status", { text: "live", online: true });
    sendSubscribe();
  };
  S.ws.onclose = () => {
    emit("conn-status", { text: "offline", online: false });
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
    log("NO TRACKED AGENT — paste a token");
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
  emit("mode-changed", S.humanControl);
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

export function setHumanControl(flag) {
  S.humanControl = flag;
  emit("mode-changed", S.humanControl);
  // /api/control is operator-only: the server requires the dev token.
  // Paste the dev token into the TRACK panel to enable this switch.
  const token = inspectToken();
  const msg = { type: "control", human_control: flag, token };
  if (S.ws && S.ws.readyState === WebSocket.OPEN) {
    S.ws.send(JSON.stringify(msg));
  } else {
    fetch("/api/control", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ human_control: flag, token }),
    }).catch(() => {});
  }
}

/** Raw message API — parsed JSON in, parsed JSON out. UI lives in dispatch. */
export async function apiSendMessage(token, targets, text) {
  const r = await fetch("/api/message", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ token, targets, text }),
  });
  return r.json();
}

export async function apiMessageStatus(token, ids) {
  const r = await fetch(
    `/api/message/status?token=${encodeURIComponent(token)}&ids=${ids.join(",")}`,
  );
  return r.json();
}

export async function apiOperatorInbox(token) {
  const r = await fetch("/api/message/inbox?token=" + encodeURIComponent(token));
  return r.json();
}

// ---------------------------------------------------------------- inspect fetches

export async function fetchEntityInspect(id) {
  const token = inspectToken();
  if (!token) {
    log("INSPECT: track a token first");
    return;
  }
  try {
    const r = await fetch(`/api/entity?id=${id}&token=` + encodeURIComponent(token));
    const d = await r.json();
    if (!d.ok) {
      log("INSPECT: " + (d.reason || "failed"));
      return;
    }
    emit("inspect-show", { kind: "entity", data: d.entity });
  } catch (_) {
    log("INSPECT: network");
  }
}

export async function fetchCellInspect(wx, wy) {
  const token = inspectToken();
  if (!token) {
    log("INSPECT: track a token first");
    return;
  }
  try {
    const r = await fetch(
      `/api/cell?x=${wx}&y=${wy}&token=` + encodeURIComponent(token),
    );
    const d = await r.json();
    if (!d.ok) {
      log("INSPECT: " + (d.reason || "failed"));
      return;
    }
    emit("inspect-show", { kind: "cell", data: d.cell });
  } catch (_) {
    log("INSPECT: network");
  }
}

on("request-inspect-entity", (id) => fetchEntityInspect(id));
on("request-inspect-cell", ({ x, y }) => fetchCellInspect(x, y));
