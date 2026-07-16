# RTS Box Selector + Custom Prompt Messaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let any spectator with a tracked token drag-select visible agents in the frontend, compose or load saved prompt templates, and broadcast those prompts to the selected agents through a new `/api/message` endpoint; agents then receive those messages in their `/api/me` runtime response.

**Architecture:** Add an `AgentMailbox` component and a `MessageCounter` resource to the ECS. Expose a visibility-gated `POST /api/message` endpoint that only delivers to agents the sender can currently see (reusing `player_visible_map`). Return unread messages in `/api/me` and mark them delivered. Upgrade the viewer to include **all** visible agents, not just the sender's own tracked agents. In the frontend, add Shift-drag box selection, Ctrl-click single selection, a prompt preset manager in `localStorage`, and a send panel.

**Tech Stack:** Rust (Bevy ECS), axum 0.8, serde_json, ROT.js, vanilla JS, localStorage.

## Global Constraints

- No persistent accounts: sender identity is `token` + optional `sender_ip` only.
- Messages are delivered only to agents inside the sender's current FOV/memory gate (same rule as `/api/entity` and `/api/cell`).
- Prompt templates are saved in browser `localStorage`; the server only sees the final text at send time.
- Agent clients decide whether to trust a prompt using their own pre-shared passphrase / IP checks; the kernel does not validate message semantics.
- Max inbox size per agent: 32 messages; older unread messages are dropped on overflow.
- Max message text length: 500 UTF-16 code units.

---

## Task 1: Add `AgentMailbox` component and `MessageCounter` resource

**Files:**
- Modify: `crates/ask-kernel/src/components.rs`
- Modify: `crates/ask-kernel/src/world.rs`
- Test: unit test in `crates/ask-kernel/src/components.rs`

**Interfaces:**
- Consumes: none
- Produces:
  - `pub struct Envelope { id: u64, from: String, text: String, sent_tick: u64, read: bool }`
  - `pub struct AgentMailbox { messages: Vec<Envelope> }` with `new()`, `push(env)`, `unread()`, `mark_read(ids)`.
  - `#[derive(Resource, Default)] pub struct MessageCounter(pub u64);`

- [ ] **Step 1: Write the failing test**

Add this `#[cfg(test)]` block at the bottom of `crates/ask-kernel/src/components.rs` (before the existing module closing brace):

```rust
#[cfg(test)]
mod mailbox_tests {
    use super::*;

    #[test]
    fn mailbox_keeps_unread_and_caps_at_32() {
        let mut mb = AgentMailbox::new();
        for i in 0..40 {
            mb.push(Envelope {
                id: 100 + i as u64,
                from: "anon".into(),
                text: format!("msg {i}"),
                sent_tick: i as u64,
                read: false,
            });
        }
        assert_eq!(mb.messages.len(), 32);
        // oldest messages dropped on overflow
        assert_eq!(mb.messages[0].id, 108);
        assert_eq!(mb.unread().len(), 32);
        mb.mark_read(&[108, 109]);
        assert_eq!(mb.unread().len(), 30);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo test -p ask-kernel mailbox_tests -- --nocapture
```

Expected: compile errors for `AgentMailbox`, `Envelope`, `MessageCounter` undefined.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/ask-kernel/src/components.rs` after the `VisionMemory` impl block (around line 320):

```rust
/// One message delivered to an agent from an external player/spectator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub id: u64,
    pub from: String,
    pub text: String,
    pub sent_tick: u64,
    pub read: bool,
}

/// Per-agent inbox. Lives on every entity with `Agent`.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentMailbox {
    pub messages: Vec<Envelope>,
}

impl AgentMailbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a message, dropping oldest if the cap is exceeded.
    pub fn push(&mut self, env: Envelope) {
        const CAP: usize = 32;
        self.messages.push(env);
        if self.messages.len() > CAP {
            let drop = self.messages.len() - CAP;
            self.messages.drain(0..drop);
        }
    }

    pub fn unread(&self) -> Vec<&Envelope> {
        self.messages.iter().filter(|m| !m.read).collect()
    }

    pub fn mark_read(&mut self, ids: &[u64]) {
        for m in &mut self.messages {
            if ids.contains(&m.id) {
                m.read = true;
            }
        }
    }
}
```

Add the resource in the same file near other resources (e.g., after `VisionMemory`):

```rust
/// Global monotonic id source for Envelopes.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MessageCounter(pub u64);
```

In `crates/ask-kernel/src/world.rs`:

1. Add `AgentMailbox` and `MessageCounter` to the imports:

```rust
use crate::components::{
    Agent, AgentMailbox, AgentProfile, Glyph, Health, Inventory, Item, MessageCounter, Monster,
    Position, Resource, ResourceKind, StableId, VisionMemory,
};
```

2. In `KernelWorld::new`, insert the resource:

```rust
world.insert_resource(MessageCounter(0));
```

3. In `spawn_level_entities`, add `AgentMailbox::new()` to the initial agent spawn bundle:

```rust
self.world.spawn((
    Agent,
    AgentMailbox::new(),
    Position { x: agent.0, y: agent.1 },
    Glyph('A'),
    Inventory::default(),
    Health::default(),
    VisionMemory::new(vw, vh),
    StableId(id),
));
```

4. In `change_level`, add `AgentMailbox::new()` to the respawned agent bundle:

```rust
self.world.spawn((
    Agent,
    AgentMailbox::new(),
    Position {
        x: agent_pos.0,
        y: agent_pos.1,
    },
    Glyph('A'),
    inv,
    hp,
    VisionMemory::new(vw, vh),
    StableId(sid),
));
```

5. In `spawn_agent`, add `AgentMailbox::new()` to the spawn bundle:

```rust
self.world.spawn((
    Agent,
    AgentMailbox::new(),
    AgentProfile { name, purpose },
    Position { x, y },
    Glyph(glyph),
    Inventory::default(),
    Health::default(),
    VisionMemory::new(vw, vh),
    StableId(id),
));
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo test -p ask-kernel mailbox_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/components.rs crates/ask-kernel/src/world.rs
git commit -m "feat(mailbox): AgentMailbox + MessageCounter for agent messaging"
```

---

## Task 2: Expose message send endpoint and return messages in `/api/me`

**Files:**
- Modify: `crates/ask-kernel/src/serve.rs`
- Modify: `crates/ask-kernel/src/main.rs` (to enable `ConnectInfo` for sender IP)
- Test: manual curl end-to-end after the full build

**Interfaces:**
- Consumes: `AgentMailbox`, `MessageCounter`, `player_visible_map(...)` from `serve.rs`
- Produces:
  - `POST /api/message` — body `{ token: String, targets: Vec<u64>, text: String }`, response `{ ok, sent, rejected, reason? }`
  - `GET /api/me?token=...` response now includes `"messages": [{ id, from, text, sent_tick }]`

- [ ] **Step 1: Add request/response types and route**

In `crates/ask-kernel/src/serve.rs`, add after `CellQuery`:

```rust
#[derive(Debug, Deserialize)]
struct MessageSendRequest {
    token: String,
    targets: Vec<u64>,
    text: String,
}

#[derive(Debug, Serialize)]
struct MessageSendResponse {
    ok: bool,
    sent: usize,
    rejected: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}
```

Register the route in `run_server`:

```rust
.route("/api/message", post(api_message_send))
```

- [ ] **Step 2: Implement `api_message_send`**

Add this handler after `api_cell`:

```rust
async fn api_message_send(
    State(st): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<MessageSendRequest>,
) -> impl IntoResponse {
    const MAX_LEN: usize = 500;
    let text = req.text.trim().to_string();
    if text.is_empty() {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("empty text".into()),
        });
    }
    if text.len() > MAX_LEN {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("text too long".into()),
        });
    }

    let mut sim = st.sim.lock().await;
    let Some(vis) = player_visible_map(&mut sim.kernel.world, &st.reg, &req.token) else {
        return Json(MessageSendResponse {
            ok: false,
            sent: 0,
            rejected: 0,
            reason: Some("unauthorized".into()),
        });
    };

    let is_dev = req
        .token
        .split(',')
        .map(str::trim)
        .any(|t| st.reg.is_dev_token(t));
    let sender_ip = addr.ip().to_string();
    let tick = sim.kernel.tick();

    // Acquire a contiguous id range.
    let base_id = {
        let mut counter = sim.kernel.world.resource_mut::<crate::components::MessageCounter>();
        let id = counter.0;
        counter.0 += req.targets.len() as u64;
        id
    };

    let mut sent = 0;
    let mut rejected = 0;
    let mut next_id = base_id;

    for target_id in req.targets {
        let found = {
            let mut q = sim
                .kernel
                .world
                .query::<(Entity, &StableId, &Position, &Agent)>();
            q.iter(&sim.kernel.world)
                .find(|(_, sid, _, _)| sid.0 == target_id)
                .map(|(e, _, p, _)| (e, p.x, p.y))
        };

        let Some((entity, x, y)) = found else {
            rejected += 1;
            continue;
        };

        if !is_dev && !vis.is_visible(x, y) {
            rejected += 1;
            continue;
        }

        let Some(mut mailbox) = sim.kernel.world.get_mut::<crate::components::AgentMailbox>(entity)
        else {
            rejected += 1;
            continue;
        };

        mailbox.push(crate::components::Envelope {
            id: next_id,
            from: sender_ip.clone(),
            text: text.clone(),
            sent_tick: tick,
            read: false,
        });
        next_id += 1;
        sent += 1;
    }

    Json(MessageSendResponse {
        ok: true,
        sent,
        rejected,
        reason: None,
    })
}
```

Also add `ConnectInfo` to the imports at the top of `serve.rs`:

```rust
use axum::extract::connect_info::ConnectInfo;
```

- [ ] **Step 3: Update `/api/me` to return unread messages and mark them delivered**

Modify `api_me` in `crates/ask-kernel/src/serve.rs`. After finding the agent entity `a`, build the `messages` array and mark them read before returning:

```rust
let messages: Vec<serde_json::Value> = {
    let agent_entity = {
        let mut q = sim.kernel.world.query::<(Entity, &StableId)>();
        q.iter(&sim.kernel.world)
            .find(|(_, sid)| sid.0 == a.id)
            .map(|(e, _)| e)
    };
    if let Some(entity) = agent_entity {
        let unread = {
            let mb = sim
                .kernel
                .world
                .get::<crate::components::AgentMailbox>(entity);
            mb.map(|m| {
                m.unread()
                    .iter()
                    .map(|env| {
                        serde_json::json!({
                            "id": env.id,
                            "from": env.from,
                            "text": env.text,
                            "sent_tick": env.sent_tick,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        };
        let ids: Vec<u64> = unread.iter().map(|v| v["id"].as_u64().unwrap_or(0)).collect();
        if let Some(mut mb) = sim
            .kernel
            .world
            .get_mut::<crate::components::AgentMailbox>(entity)
        {
            mb.mark_read(&ids);
        }
        unread
    } else {
        Vec::new()
    }
};
```

Then add `"messages": messages` to the final `serde_json::json!({ ... })` response.

- [ ] **Step 4: Enable `ConnectInfo` in the server**

In `crates/ask-kernel/src/main.rs`, change the final serve call from:

```rust
axum::serve(listener, app).await?;
```

to:

```rust
use std::net::SocketAddr;
axum::serve(
    listener,
    app.into_make_service_with_connect_info::<SocketAddr>(),
)
.await?;
```

- [ ] **Step 5: Build and run a manual smoke test**

Build:

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo build --release -p ask-kernel
```

Run the server locally, register an agent, then send a message with the dev token:

```bash
# terminal 1
./target/release/ask-kernel --serve --port 8080

# terminal 2: register
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/api/register \
  -H 'Content-Type: application/json' \
  -d '{"name":"Scout","purpose":"test"}' | jq -r '.token')

# send a prompt to that agent (replace AGENT_ID with the returned id)
curl -s -X POST http://127.0.0.1:8080/api/message \
  -H 'Content-Type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"targets\":[AGENT_ID],\"text\":\"gather wood and build a hut\"}"

# poll me; expect messages array
curl -s "http://127.0.0.1:8080/api/me?token=$TOKEN" | jq '.messages'
```

Expected: the agent's `/api/me` returns one message with `id`, `from` (ip), `text`, `sent_tick`.

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/src/serve.rs crates/ask-kernel/src/main.rs
git commit -m "feat(message): POST /api/message and messages in /api/me"
```

---

## Task 3: Show all visible agents in the snapshot (not just tracked ones)

**Files:**
- Modify: `crates/ask-kernel/src/viewer.rs`
- Test: visual/manual after frontend upgrade

**Interfaces:**
- Consumes: `VisionMap::is_visible`, `allowed_agents: Option<&[u64]>`
- Produces: `ViewerSnapshot.entities` includes any visible agent, plus own tracked agents even when not currently visible

- [ ] **Step 1: Modify the agent query condition**

In `crates/ask-kernel/src/viewer.rs`, find the agent entity query block and move the `can_see` closure above it. Replace:

```rust
let mut entities = Vec::new();
{
    let mut q = world.query::<(
        &StableId,
        &Position,
        &Glyph,
        &Inventory,
        &Health,
        Option<&AgentProfile>,
        &Agent,
    )>();
    for (id, p, g, inv, hp, profile, _) in q.iter(world) {
        if let Some(allowed) = allowed_agents {
            if !allowed.contains(&id.0) {
                continue;
            }
        }
```

with:

```rust
let can_see = |x: i32, y: i32| -> bool { vis.is_visible(x, y) };

let mut entities = Vec::new();
{
    let mut q = world.query::<(
        &StableId,
        &Position,
        &Glyph,
        &Inventory,
        &Health,
        Option<&AgentProfile>,
        &Agent,
    )>();
    for (id, p, g, inv, hp, profile, _) in q.iter(world) {
        if let Some(allowed) = allowed_agents {
            // Own tracked agents are always shown; any other agent is shown only if visible now.
            if !allowed.contains(&id.0) && !can_see(p.x, p.y) {
                continue;
            }
        }
```

Then remove the later duplicate `let can_see = ...` line (it appears before non-agent queries).

- [ ] **Step 2: Build and verify no regressions**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo test -p ask-kernel
```

Expected: all existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ask-kernel/src/viewer.rs
git commit -m "feat(viewer): include all visible agents, not just tracked ones"
```

---

## Task 4: Frontend RTS box selection

**Files:**
- Modify: `crates/ask-kernel/static/index.html`
- Modify: `crates/ask-kernel/static/app.css`
- Modify: `crates/ask-kernel/static/app.js`
- Test: manual browser test

**Interfaces:**
- Consumes: `lastSnap.entities`, `cam`, `cellSize()`, `tracked[0].token`
- Produces:
  - `selectedAgentIds: Set<number>`
  - `startBoxSelect(e)`, `updateBoxSelect(e)`, `finishBoxSelect(e)`
  - `toggleSelectAgent(id)` / `setSelectedAgents(ids)`
  - visual selection highlight inside `drawSnap`

- [ ] **Step 1: Add the selection box DOM element**

In `crates/ask-kernel/static/index.html`, add inside `#viewport`:

```html
<div id="viewport">
  <div id="select-box"></div>
  <div id="map"></div>
</div>
```

- [ ] **Step 2: Add selection styles**

In `crates/ask-kernel/static/app.css`, add:

```css
#select-box {
  position: absolute;
  border: 1px solid var(--r80-accent);
  background: rgba(0, 255, 255, 0.12);
  pointer-events: none;
  z-index: 3;
  display: none;
}
#select-box.active {
  display: block;
}
```

- [ ] **Step 3: Add selection state and helpers in `app.js`**

Add DOM refs near the top:

```javascript
const elSelectBox = document.getElementById("select-box");
```

Add state after `let followToken = ...`:

```javascript
let selecting = false;
let selectStart = null; // { sx, sy, wx, wy }
let selectedAgentIds = new Set();
```

Add helper functions after `cellSize()`:

```javascript
function worldAtScreen(clientX, clientY) {
  const mapRect = elMap.getBoundingClientRect();
  const cs = cellSize();
  const sx = clientX - mapRect.left;
  const sy = clientY - mapRect.top;
  return {
    wx: Math.floor(cam.tx + sx / cs),
    wy: Math.floor(cam.ty + sy / cs),
    sx,
    sy,
  };
}

function updateSelectionHighlight() {
  // drawSnap already reads selectedAgentIds globally
  if (lastSnap) drawSnap(lastSnap);
}

function setSelectedAgents(ids) {
  selectedAgentIds = new Set(ids);
  updateSelectionPanel();
  updateSelectionHighlight();
}

function toggleSelectAgent(id) {
  if (selectedAgentIds.has(id)) {
    selectedAgentIds.delete(id);
  } else {
    selectedAgentIds.add(id);
  }
  updateSelectionPanel();
  updateSelectionHighlight();
}

function startBoxSelect(e) {
  const { wx, wy, sx, sy } = worldAtScreen(e.clientX, e.clientY);
  selecting = true;
  selectStart = { sx, sy, wx, wy };
  elSelectBox.style.left = sx + "px";
  elSelectBox.style.top = sy + "px";
  elSelectBox.style.width = "0px";
  elSelectBox.style.height = "0px";
  elSelectBox.classList.add("active");
}

function updateBoxSelect(e) {
  if (!selecting || !selectStart) return;
  const { sx, sy } = worldAtScreen(e.clientX, e.clientY);
  const left = Math.min(selectStart.sx, sx);
  const top = Math.min(selectStart.sy, sy);
  const width = Math.abs(selectStart.sx - sx);
  const height = Math.abs(selectStart.sy - sy);
  elSelectBox.style.left = left + "px";
  elSelectBox.style.top = top + "px";
  elSelectBox.style.width = width + "px";
  elSelectBox.style.height = height + "px";
}

function finishBoxSelect(e) {
  if (!selecting || !selectStart) return;
  selecting = false;
  elSelectBox.classList.remove("active");

  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const x0 = Math.min(selectStart.wx, wx);
  const x1 = Math.max(selectStart.wx, wx);
  const y0 = Math.min(selectStart.wy, wy);
  const y1 = Math.max(selectStart.wy, wy);

  const ids = (lastSnap ? lastSnap.entities : [])
    .filter(
      (en) =>
        en.kind === "agent" &&
        en.x >= x0 &&
        en.x <= x1 &&
        en.y >= y0 &&
        en.y <= y1,
    )
    .map((en) => en.id);

  if (e.ctrlKey) {
    for (const id of ids) toggleSelectAgent(id);
  } else {
    setSelectedAgents(ids);
  }
  pushLog(`SELECTED ${selectedAgentIds.size} agents`);
}
```

- [ ] **Step 4: Wire mouse events for selection vs pan/inspect**

Change the `mousedown` handler:

```javascript
elViewport.addEventListener("mousedown", (e) => {
  if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;

  if (e.shiftKey && e.button === 0) {
    e.preventDefault();
    startBoxSelect(e);
    return;
  }

  dragging = true;
  dragLast = { x: e.clientX, y: e.clientY };
  accumX = 0;
  accumY = 0;
  dragPixelDist = 0;
  mouseDownAt = {
    x: e.clientX,
    y: e.clientY,
    t: performance.now(),
    button: e.button,
    shiftKey: e.shiftKey,
    ctrlKey: e.ctrlKey,
  };
  elViewport.classList.add("dragging");
  cam.follow = false;
  e.preventDefault();
});
```

Update `mousemove` to call `updateBoxSelect`:

```javascript
window.addEventListener("mousemove", (e) => {
  if (selecting) {
    updateBoxSelect(e);
    return;
  }
  if (!dragging || !dragLast) return;
  // ... existing drag code unchanged ...
});
```

Update `mouseup`:

```javascript
window.addEventListener("mouseup", (e) => {
  if (selecting) {
    finishBoxSelect(e);
    return;
  }
  if (!dragging) return;
  dragging = false;
  dragLast = null;
  elViewport.classList.remove("dragging");
  const down = mouseDownAt;
  mouseDownAt = null;
  if (down) {
    const dt = performance.now() - down.t;
    if (dragPixelDist < 6 && dt < 450) {
      if (down.ctrlKey) {
        handleSelectClick(e);
      } else {
        handleInspectClick(e, down.button);
      }
    }
  }
});
```

Add `handleSelectClick`:

```javascript
function handleSelectClick(e) {
  if (!lastSnap || mapW <= 0 || mapH <= 0) return;
  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const ent = (lastSnap.entities || []).find(
    (en) => en.kind === "agent" && en.x === wx && en.y === wy,
  );
  if (ent) {
    toggleSelectAgent(ent.id);
  }
}
```

- [ ] **Step 5: Highlight selected agents in `drawSnap`**

In the agent rendering loop, change the background when selected:

```javascript
const SELECT_BG = theme.selection || "#003333";
// ... inside the entity loop, before d.draw ...
let bg = e.entityBg;
if (ent.kind === "agent" && selectedAgentIds.has(ent.id)) {
  bg = SELECT_BG;
}
d.draw(vx, vy, glyph, fg, bg);
```

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/static/index.html crates/ask-kernel/static/app.css crates/ask-kernel/static/app.js
git commit -m "feat(ui): RTS Shift-drag + Ctrl-click agent selection"
```

---

## Task 5: Prompt presets and send panel

**Files:**
- Modify: `crates/ask-kernel/static/index.html`
- Modify: `crates/ask-kernel/static/app.css`
- Modify: `crates/ask-kernel/static/app.js`
- Test: manual end-to-end with `/api/message`

**Interfaces:**
- Consumes: `selectedAgentIds`, `inspectToken()`, `fetch()`
- Produces:
  - `loadPresets()`, `savePreset(name, text)`, `deletePreset(id)`
  - `updateSelectionPanel()`
  - `sendPromptToSelected(text)`

- [ ] **Step 1: Add the selector panel HTML**

In `crates/ask-kernel/static/index.html`, add after `#theme-bar`:

```html
<div id="selector-panel">
  <div class="title">+ SELECTOR +</div>
  <div class="muted" id="sel-count">0 agents selected</div>
  <div class="row">
    <select id="sel-preset" class="term-input"></select>
    <button type="button" id="sel-preset-del" class="term-btn secondary" title="Delete preset">[x]</button>
  </div>
  <textarea
    id="sel-text"
    class="term-input"
    rows="4"
    placeholder="Type a prompt to send to selected agents..."
  ></textarea>
  <div class="row">
    <input
      id="sel-preset-name"
      class="term-input"
      type="text"
      placeholder="preset name"
    />
    <button type="button" id="sel-preset-save" class="term-btn">[SAVE]</button>
  </div>
  <button type="button" id="sel-send" class="term-btn">[ SEND ]</button>
</div>
```

- [ ] **Step 2: Add panel styles**

In `crates/ask-kernel/static/app.css`:

```css
#selector-panel {
  position: fixed;
  top: 28px;
  right: calc(var(--space-sm) + 200px + var(--space-sm));
  z-index: 5;
  width: min(280px, 34vw);
  max-height: calc(100vh - 80px);
  overflow: auto;
  background: var(--r80-neutral);
  border: 1px solid var(--r80-text);
  padding: var(--space-sm);
  pointer-events: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
}
#selector-panel .title {
  color: var(--r80-accent);
  font-weight: 700;
  letter-spacing: 0.06em;
}
#selector-panel textarea.term-input {
  resize: vertical;
  min-height: 60px;
}
```

- [ ] **Step 3: Add preset/selection logic in `app.js`**

Add DOM refs:

```javascript
const elSelCount = document.getElementById("sel-count");
const elSelPreset = document.getElementById("sel-preset");
const elSelPresetDel = document.getElementById("sel-preset-del");
const elSelPresetSave = document.getElementById("sel-preset-save");
const elSelPresetName = document.getElementById("sel-preset-name");
const elSelText = document.getElementById("sel-text");
const elSelSend = document.getElementById("sel-send");

const PRESETS_KEY = "ask-presets-v1";
```

Add functions after `updateSelectionHighlight`:

```javascript
function loadPresets() {
  try {
    const raw = JSON.parse(localStorage.getItem(PRESETS_KEY) || "[]");
    return Array.isArray(raw) ? raw : [];
  } catch (_) {
    return [];
  }
}

function savePresets(presets) {
  localStorage.setItem(PRESETS_KEY, JSON.stringify(presets));
}

function renderPresets() {
  if (!elSelPreset) return;
  const presets = loadPresets();
  elSelPreset.innerHTML = "";
  const none = document.createElement("option");
  none.value = "";
  none.textContent = "-- preset --";
  elSelPreset.appendChild(none);
  for (const p of presets) {
    const opt = document.createElement("option");
    opt.value = p.id;
    opt.textContent = p.name;
    elSelPreset.appendChild(opt);
  }
}

function updateSelectionPanel() {
  if (!elSelCount) return;
  elSelCount.textContent = `${selectedAgentIds.size} agents selected`;
}

async function sendPromptToSelected(text) {
  const token = inspectToken();
  if (!token) {
    pushLog("SEND: track a token first");
    return;
  }
  if (!selectedAgentIds.size) {
    pushLog("SEND: select agents first");
    return;
  }
  if (!text.trim()) {
    pushLog("SEND: empty prompt");
    return;
  }
  const targets = Array.from(selectedAgentIds);
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
    pushLog(`SEND → ${d.sent} agents, ${d.rejected} rejected`);
  } catch (_) {
    pushLog("SEND: network");
  }
}
```

Bind events near the bottom:

```javascript
if (elSelPreset) {
  elSelPreset.addEventListener("change", () => {
    const id = elSelPreset.value;
    if (!id) return;
    const p = loadPresets().find((x) => x.id === id);
    if (p && elSelText) elSelText.value = p.text;
  });
}
if (elSelPresetSave) {
  elSelPresetSave.addEventListener("click", () => {
    const name = (elSelPresetName && elSelPresetName.value || "").trim();
    const text = (elSelText && elSelText.value || "").trim();
    if (!name || !text) {
      pushLog("PRESET: need name and text");
      return;
    }
    const presets = loadPresets();
    presets.push({ id: Date.now().toString(36), name, text });
    savePresets(presets);
    renderPresets();
    if (elSelPresetName) elSelPresetName.value = "";
    pushLog(`PRESET saved: ${name}`);
  });
}
if (elSelPresetDel) {
  elSelPresetDel.addEventListener("click", () => {
    const id = elSelPreset.value;
    if (!id) return;
    const presets = loadPresets().filter((p) => p.id !== id);
    savePresets(presets);
    renderPresets();
    if (elSelText) elSelText.value = "";
    pushLog("PRESET deleted");
  });
}
if (elSelSend) {
  elSelSend.addEventListener("click", () => {
    sendPromptToSelected(elSelText ? elSelText.value : "");
  });
}
```

Call `renderPresets(); updateSelectionPanel();` at the end of the startup block (next to `renderTracker()`).

- [ ] **Step 4: Update help text**

In `index.html`:

```html
<div id="help">
  KEYS: arrows move · g interact · t dig · u scoop · v place · n plant · b build · x decon · y craft · m mock · SPACE follow · click inspect · right-click cell · Shift-drag select · Ctrl-click toggle
</div>
```

- [ ] **Step 5: Manual end-to-end test**

1. Open `http://111.231.50.85:8000`.
2. Paste your agent token (or the dev token).
3. Hold Shift and drag a box around visible agents.
4. Type a prompt or choose a saved preset.
5. Click `[ SEND ]`.
6. Poll `GET /api/me?token=<agent-token>` with curl; expect `messages` array containing the prompt.

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/static/index.html crates/ask-kernel/static/app.css crates/ask-kernel/static/app.js
git commit -m "feat(ui): prompt presets and send panel for selected agents"
```

---

## Task 6: Update skill documentation

**Files:**
- Modify: `.claude/skills/ask-sandbox/SKILL.md`
- Test: read the file to confirm the new sections render

- [ ] **Step 1: Add the new API endpoints and concept**

Insert after the Actions section in `.claude/skills/ask-sandbox/SKILL.md`:

```markdown
## Messages (RTS selector)

Any spectator with a tracked token can select visible agents in the web UI and send them a custom prompt. Agents receive those prompts inside `/api/me` exactly once.

```bash
# send a prompt to one or more agents (targets are StableId values)
curl -s -X POST http://111.231.50.85:8000/api/message \
  -H 'Content-Type: application/json' \
  -d '{"token":"ask1_...","targets":[7,12],"text":"build a hut"}'

# agent runtime polls me and sees messages
curl -s 'http://111.231.50.85:8000/api/me?token=ask1_...' | jq '.messages'
```

Your agent client should inspect `.messages[]` and decide whether to obey based on its own passphrase or sender IP checks. The kernel only guarantees visibility: a sender cannot message an agent it cannot currently see.
```

- [ ] **Step 2: Commit**

```bash
git add .claude/skills/ask-sandbox/SKILL.md
git commit -m "docs(skill): document /api/message and RTS selector"
```

---

## Task 7: Build, package, and deploy

**Files:**
- Build artifact: `target/release/ask-kernel`
- Package: `ask-sandbox-skill.tar.gz`
- Test: remote curl smoke tests

- [ ] **Step 1: Full test suite and release build locally**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo test -p ask-kernel
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo build --release -p ask-kernel
```

Expected: tests pass, binary built.

- [ ] **Step 2: Package the skill**

```bash
tar czf ask-sandbox-skill.tar.gz -C .claude/skills ask-sandbox
```

- [ ] **Step 3: Deploy to remote host**

```bash
tar czf /tmp/ask-kernel-code.tar.gz --exclude='target' --exclude='.git' --exclude='.claude' .
sshpass -p 'Odyssey20031213' scp -o StrictHostKeyChecking=no /tmp/ask-kernel-code.tar.gz root@111.231.50.85:/tmp/ask-kernel-code.tar.gz
sshpass -p 'Odyssey20031213' ssh -o StrictHostKeyChecking=no root@111.231.50.85 \
  'cd /root/AgentGame && tar xzf /tmp/ask-kernel-code.tar.gz --overwrite && source $HOME/.cargo/env && cargo build --release -p ask-kernel && systemctl restart ask-kernel'
```

- [ ] **Step 4: Remote smoke tests**

```bash
# dev token snapshot shows all agents, including other players' visible agents
DEV=ask1_dev_905aaf14cf8dba733847e7f548710dba
AGENT_ID=$(curl -s "http://111.231.50.85:8000/api/snapshot?token=$DEV" | jq '.entities[] | select(.kind=="agent") | .id' | head -1)

# send a prompt
curl -s -X POST http://111.231.50.85:8000/api/message \
  -H 'Content-Type: application/json' \
  -d "{\"token\":\"$DEV\",\"targets\":[$AGENT_ID],\"text\":\"hello agent\"}"

# verify it appears in /api/me for that agent (needs the agent's own token, not dev)
# curl -s "http://111.231.50.85:8000/api/me?token=<agent-token>" | jq '.messages'
```

- [ ] **Step 5: Commit**

```bash
git add ask-sandbox-skill.tar.gz
git commit -m "chore: package skill tarball for rts-selector-messaging"
```

---

## Self-Review

**1. Spec coverage:**
- ✅ RTS-style box selection → Task 4
- ✅ Custom prompt templates saved by the player → Task 5 presets in localStorage
- ✅ Select batch of agents and send saved prompt → Task 5 send panel + Task 2 endpoint
- ✅ Selected agents' `/api/me` runtime returns the prompt → Task 2 `messages` field
- ✅ Any player can send to any visible agent, no accounts → Task 2 visibility gate + Task 3 showing all visible agents
- ✅ Agent decides correctness via passphrase/IP → Task 2 includes `from` IP, raw text; agent client filters
- ✅ Great and simple → yes

**2. Placeholder scan:**
- No TBD/TODO/fill-in-details.
- All code blocks contain real code.
- No "similar to Task N" shortcuts.

**3. Type consistency:**
- `AgentMailbox.messages` is `Vec<Envelope>` everywhere.
- `MessageCounter.0` is `u64` everywhere.
- `POST /api/message` request uses `targets: Vec<u64>` matching `StableId.0`.
- `ViewerEntity.id` is `u64`, consistent with selection IDs.
