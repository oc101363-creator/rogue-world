# Frontend Dock Re-architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the scattered fixed-position UI (7 panels with magic-number coordinates) with a dock-grid shell + self-contained panel modules, keeping every existing feature.

**Architecture:** CSS-grid dock shell (hud / left dock / map / right dock with DISPATCH+INSPECT tabs / bottom log). Each panel is one ES module exporting `mount(root)` that owns its template, scoped queries, and listeners. Cross-module talk goes through a tiny pub/sub `bus.js` (kills the render.js ↔ net.js import cycle). `state.js` loses the 40-ref hand-synced `el` registry — DOM ids are the HTML shell's public contract; modules query their own roots.

**Tech Stack:** Vanilla ES modules, ROT.js 2.2.1 (canvas map), axum dev server for live smoke, `node --test` + a stub-based link-check script for verification.

## Problem Diagnosis (the spec)

1. **Scattered layout** — 7 `position: fixed` panels at magic coordinates; `#selector-panel` positions itself with `right: calc(... 200px ...)` hardcoding `#theme-bar`'s width. Add one button anywhere → overlap.
2. **`el` registry** — `state.js` holds 40+ `getElementById` refs synced by hand across 4 files. Adding one control means editing HTML + state.js + CSS + a wiring file.
3. **Circular imports** — `render.js` imports `sendSubscribe` from `net.js`; `net.js` imports 11 functions from `render.js`.
4. **Mixed module paradigms** — `themes.js`/`art.js` are classic scripts defining globals (`THEMES`, `getTheme`, `decodeFeatIds`, `ensureArtCatalog`); app modules are ES modules. Load order is load-bearing.
5. **`render.js` grab-bag** (563 lines) — map drawing + camera + tracker + presets + chips + delivery + inspect popup + theme chrome.
6. **Inspect popup floats over the map** at top-center, covering exactly what you are inspecting.
7. **No layout system** — no grid, no docking, no collapse, no responsive story (`min(320px, 42vw)` overlaps on small screens).

## Target Layout

```
┌──────────────────────────── #hud ────────────────────────────┐
│ status · mode · info · cam · THEME · FOLLOW · MOCK · ◀ · ▶  │
├──────────────┬──────────────────────────────┬────────────────┤
│ #dock-track  │                              │ #dock-command  │
│  tokens      │      #viewport/#map          │ tabs:          │
│  list+follow │      (fills grid cell)       │ DISPATCH|INSPECT│
├──────────────┴──────────────────────────────┴────────────────┤
│ #dock-log (grows)                             │ #help        │
└──────────────────────────────────────────────────────────────┘
```

- Zero `position: fixed`; zero magic numbers. Docks collapse via hud buttons (persisted to localStorage).
- INSPECT becomes a tab in the right dock — stays open while you watch the map; no more popup over the viewport.
- Below 1100px the docks become overlays (media query), map keeps the screen.

## File Structure (final state)

```
static/
  index.html        grid shell ONLY (ids are its public contract)
  app.css           grid layout + term-* primitives + theme vars
  app.js            boot: mount panels, install input, connect
  state.js          S + storage keys + pure helpers + selection mutators (NO DOM)
  bus.js            on/emit/log — the only cross-module channel
  themes.js         ES module: export THEMES, getTheme
  art.js            ES module: export decodeFeatIds, lookForFeat, lookForEntity,
                    materialColor, ensureArtCatalog
  mapview.js        ROT display, camera, drawSnap, select overlay, coord queries
  net.js            server comms + raw API fns; emits bus events; no render imports
  input.js          mouse/keyboard → intents (mapview + state + bus + net)
  panels/hud.js     status/mode/info/cam + theme select + FOLLOW/MOCK + dock toggles
  panels/tracker.js token add/clear + tracked list + follow-on-click
  panels/dispatch.js chips, squads, presets, send, delivery rows, op inbox
  panels/inspect.js docked inspect tab (entity/cell renderKV)
  panels/logview.js event log + help line
tests/frontend/     node --test suites (bus, state helpers)
scripts/check-frontend.mjs  stub-based module link check
```

**Panel contract:** a panel exports `mount(root)`. It owns everything under `root`: template HTML, scoped `root.querySelector`, event listeners, rendering. Panels never import each other; they read `S`, call state mutators, import raw API from `net.js`, and talk cross-module only via `bus.js`.

**Bus events (the whole contract):**

| event | emitter | subscribers | payload |
|---|---|---|---|
| `snapshot` | net | mapview, dispatch (chip dim) | ViewerSnapshot |
| `events` | net | logview | GameEvent[] (pre-filtered) |
| `log` | anyone (via `log()`) | logview | string |
| `conn-status` | net | hud | `{text, online}` |
| `hud-info` | net | hud | info string |
| `mode-changed` | net | hud | `human_control: bool` |
| `camera-changed` | mapview | hud | — |
| `theme-changed` | hud | mapview, hud | — |
| `tracked-changed` | net, tracker | tracker, net (resubscribe) | — |
| `selection-changed` | state mutators | dispatch, mapview | — |
| `inspect-show` | net (fetch results) | inspect, tabs | `{title, html}` |
| `request-inspect-entity` | input | net | `id: u64` |
| `request-inspect-cell` | input | net | `{x, y}` |
| `activate-tab` | input, inspect | tabs | `"dispatch"` \| `"inspect"` |

## Global Constraints

- No build step, no npm deps, no framework. Vanilla ES modules + ROT.js from CDN.
- Every existing feature survives: track/follow, mock toggle, themes, box select, all-vis select, squads, presets, send + delivery receipts, op inbox, inspect entity/cell, camera pan/zoom, keyboard commands, event log.
- No new gameplay features. This is a re-architecture, not a feature drop.
- Panels never import each other; cross-module traffic only via `bus.js` events listed above. New events require extending the table in this plan's successor doc.
- `state.js` contains zero DOM references after Task 9.
- Server side is untouched except `Serve::build` static-dir caching (no change needed — same files, same paths).
- Verification per task: `node --test tests/frontend/` + `node scripts/check-frontend.mjs` must pass; visual tasks add a live-server smoke step.
- Commit after every task.

---

### Task 1: bus.js + frontend test harness

**Files:**
- Create: `crates/ask-kernel/static/bus.js`
- Create: `scripts/check-frontend.mjs`
- Create: `tests/frontend/bus.test.mjs`

**Interfaces:**
- Produces: `on(event: string, fn: (payload?) => void): () => void`, `emit(event: string, payload?): void`, `log(msg: string): void` — used by every later task.

- [ ] **Step 1: Write the failing test**

```js
// tests/frontend/bus.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { on, emit, log } from "../../crates/ask-kernel/static/bus.js";

test("on/emit delivers payload to subscribers", () => {
  const seen = [];
  on("snapshot", (s) => seen.push(s.tick));
  emit("snapshot", { tick: 42 });
  assert.deepEqual(seen, [42]);
});

test("unsubscribe stops delivery", () => {
  const seen = [];
  const off = on("log", (m) => seen.push(m));
  off();
  emit("log", "nope");
  assert.deepEqual(seen, []);
});

test("log() emits the log event", () => {
  const seen = [];
  on("log", (m) => seen.push(m));
  log("hello");
  assert.deepEqual(seen, ["hello"]);
});

test("a throwing listener does not break other listeners", () => {
  const seen = [];
  on("x", () => { throw new Error("boom"); });
  on("x", () => seen.push(1));
  emit("x");
  assert.deepEqual(seen, [1]);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test tests/frontend/`
Expected: FAIL — `Cannot find module '.../static/bus.js'`

- [ ] **Step 3: Write minimal implementation**

```js
// crates/ask-kernel/static/bus.js
/* Tiny pub/sub — the ONLY cross-module channel besides state.js.
 * Breaks the old render.js ↔ net.js import cycle: producers emit,
 * consumers subscribe, neither imports the other. */

const listeners = new Map(); // event -> Set<fn>

/** Subscribe. Returns the unsubscribe function. */
export function on(event, fn) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event).add(fn);
  return () => listeners.get(event)?.delete(fn);
}

/** Publish. A throwing listener is logged and skipped, never fatal. */
export function emit(event, payload) {
  for (const fn of listeners.get(event) ?? []) {
    try {
      fn(payload);
    } catch (e) {
      console.error(`[bus] listener for "${event}" threw:`, e);
    }
  }
}

/** Every "pushLog" call site in the old code becomes this. */
export function log(msg) {
  emit("log", msg);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test tests/frontend/`
Expected: `pass 4`

- [ ] **Step 5: Write the link-check harness**

```js
// scripts/check-frontend.mjs
/* Module link check: import every frontend module with browser globals
 * stubbed. Catches bad import paths, missing exports, and top-level
 * reference errors — the frontend's compile step. */
const elStub = () => ({
  style: { setProperty() {} },
  classList: { add() {}, remove() {}, toggle() {} },
  appendChild() {}, addEventListener() {},
  set innerHTML(v) {}, get innerHTML() { return ""; },
  textContent: "", value: "", getContext: () => null,
});
globalThis.location = { protocol: "http:", host: "x" };
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
globalThis.document = {
  getElementById: elStub, createElement: elStub,
  documentElement: { style: { setProperty() {} } }, body: { style: {} },
};
globalThis.window = { addEventListener() {} };
globalThis.requestAnimationFrame = () => {};

const base = new URL("../crates/ask-kernel/static/", import.meta.url);
const modules = process.argv.slice(2);
if (!modules.length) {
  console.error("usage: node scripts/check-frontend.mjs <module.js>…");
  process.exit(2);
}
for (const m of modules) {
  await import(new URL(m, base));
  console.log("ok", m);
}
console.log("module graph links OK");
```

- [ ] **Step 6: Run harness on the one new module**

Run: `node scripts/check-frontend.mjs bus.js`
Expected: `ok bus.js` + `module graph links OK`

- [ ] **Step 7: Commit**

```bash
git add crates/ask-kernel/static/bus.js scripts/check-frontend.mjs tests/frontend/
git commit -m "feat(frontend): bus.js pub/sub + link-check harness (dock re-arch T1)"
```

---

### Task 2: themes.js + art.js become ES modules

**Files:**
- Modify: `crates/ask-kernel/static/themes.js`
- Modify: `crates/ask-kernel/static/art.js`
- Modify: `crates/ask-kernel/static/state.js` (import getTheme instead of global)
- Modify: `crates/ask-kernel/static/render.js` (import art + theme helpers)
- Modify: `crates/ask-kernel/static/net.js` (import ensureArtCatalog)
- Modify: `crates/ask-kernel/static/index.html` (drop classic script tags)

**Interfaces:**
- Produces: `themes.js` → `export { THEMES, getTheme }`; `art.js` → `export { decodeFeatIds, lookForFeat, lookForEntity, materialColor, ensureArtCatalog }`. Signatures unchanged.

- [ ] **Step 1: themes.js exports**

In `themes.js`: change `const THEMES = [` → `export const THEMES = [` and `function getTheme(id)` → `export function getTheme(id)`. Leave `FS_HDG_BASE_MATERIALS` private (it is only used inside themes.js).

- [ ] **Step 2: art.js exports**

In `art.js`: add `export` to `decodeFeatIds`, `lookForFeat`, `lookForEntity`, `materialColor`, and `ensureArtCatalog`. Check `featKey` — it is only used internally; leave it private.

- [ ] **Step 3: update the three consumers**

`state.js` top:
```js
import { getTheme } from "./themes.js";
```
(delete nothing else; the global call at `theme: getTheme(...)` now resolves via import)

`render.js` top — add:
```js
import { getTheme } from "./themes.js";
import { decodeFeatIds, lookForFeat, lookForEntity, materialColor } from "./art.js";
```
Check which art helpers render.js actually calls with `grep -n "decodeFeatIds\|lookForFeat\|lookForEntity\|materialColor\|getTheme" render.js` and import exactly those.

`net.js`: add `import { ensureArtCatalog } from "./art.js";` and replace both `typeof ensureArtCatalog === "function"` guards with direct calls (`ensureArtCatalog(...).catch(() => {})`) — imported bindings are always defined.

- [ ] **Step 4: drop classic script tags**

In `index.html` delete:
```html
<script src="/static/art.js?v=__ASK_VER__"></script>
<script src="/static/themes.js?v=__ASK_VER__"></script>
```
Keep `<script src="https://cdn.jsdelivr.net/npm/rot-js@2.2.1/dist/rot.js"></script>` (ROT stays a global lib).

- [ ] **Step 5: link-check + tests**

Run: `node scripts/check-frontend.mjs bus.js state.js themes.js art.js render.js net.js input.js app.js`
Expected: all `ok` + `module graph links OK`

- [ ] **Step 6: live smoke**

```bash
cargo build -p ask-kernel && ./target/debug/ask-kernel --serve --port 8090 &
curl -s localhost:8090/ | grep -c "themes.js"   # expect 0 (tag removed)
curl -s localhost:8090/static/themes.js | head -c 200  # serves as module
```
Open `http://localhost:8090/` — map renders with theme, tracker works (proves globals survived modularization).

- [ ] **Step 7: Commit**

```bash
git add crates/ask-kernel/static/
git commit -m "refactor(frontend): themes/art become ES modules, drop classic-script globals (T2)"
```

---

### Task 3: Dock-grid shell (index.html + app.css rewrite)

**Files:**
- Modify: `crates/ask-kernel/static/index.html` (full rewrite)
- Modify: `crates/ask-kernel/static/app.css` (full rewrite)

**Interfaces:**
- Produces: the DOM id contract every panel task relies on: `hud`, `dock-track`, `viewport`, `map`, `select-box`, `dock-command`, `command-tabs`, `tab-dispatch`, `tab-inspect`, `dock-log`, `help`. Legacy ids `tracker`, `selector-panel`, `log`, `theme-bar`, `inspect-popup`, `status`, `mode`, `info`, `cam`, `theme`, `btn-follow`, `btn-mock`, `token-input`, `token-add`, `token-clear`, `tracker-list`, `tracker-hint`, plus all `sel-*`/`squad-*`/`op-inbox` ids are RETAINED inside the migrated markup so legacy JS keeps working until its panel task lands.

**Layout mechanics (write exactly this):**

`index.html` full content:

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>ASK // FS-HDG</title>
    <link rel="stylesheet" href="/static/app.css?v=__ASK_VER__" />
    <script src="https://cdn.jsdelivr.net/npm/rot-js@2.2.1/dist/rot.js"></script>
  </head>
  <body>
    <header id="hud">
      <span id="status">…</span>
      <span id="mode" class="mode-mock">MOCK</span>
      <span id="info"></span>
      <span id="cam"></span>
      <span class="spacer"></span>
      <div id="theme-bar">
        <label for="theme">THEME</label>
        <select id="theme" aria-label="Map color theme"></select>
        <button type="button" id="btn-follow" class="term-btn" title="Follow focused agent">[ FOLLOW ]</button>
        <button type="button" id="btn-mock" class="term-btn secondary" title="Toggle mock">[ MOCK ]</button>
        <button type="button" id="dock-toggle-l" class="term-btn secondary" title="Toggle left dock">[ ◀ ]</button>
        <button type="button" id="dock-toggle-r" class="term-btn secondary" title="Toggle right dock">[ ▶ ]</button>
      </div>
    </header>

    <main id="main">
      <aside id="dock-track" class="dock">
        <!-- LEGACY panel, migrated in Task 5 — id kept for old JS -->
        <div id="tracker" aria-label="Agent tracker">
          <div class="title">+-- AGENT TRACK --+</div>
          <div class="muted">paste token to spectate</div>
          <div class="row">
            <input id="token-input" class="term-input" type="text" spellcheck="false"
                   autocomplete="off" placeholder="ask1_…" aria-label="Agent token" />
            <button type="button" id="token-add" class="term-btn" title="Add tracker">[+]</button>
            <button type="button" id="token-clear" class="term-btn secondary" title="Clear all tracked tokens">[ CLEAR ]</button>
          </div>
          <div id="tracker-list"></div>
          <div class="muted" id="tracker-hint">0 tracked (saved locally)</div>
        </div>
      </aside>

      <div id="viewport">
        <div id="select-box"></div>
        <div id="map"></div>
      </div>

      <aside id="dock-command" class="dock">
        <nav id="command-tabs">
          <button type="button" class="tab-btn active" data-tab="dispatch">DISPATCH</button>
          <button type="button" class="tab-btn" data-tab="inspect">INSPECT</button>
        </nav>
        <section id="tab-dispatch" class="tab-page active">
          <!-- LEGACY panel, migrated in Task 6 — id kept for old JS -->
          <div id="selector-panel">
            <div class="muted" id="sel-count">0 agents selected</div>
            <div class="muted">LMB drag box · double-click all visible · MMB/RMB pan</div>
            <div id="sel-chips"></div>
            <div class="row">
              <button type="button" id="sel-all-vis" class="term-btn" title="Select all agents currently in FOV">[ ALL VIS ]</button>
              <button type="button" id="sel-clear" class="term-btn secondary" title="Clear selection">[ CLEAR ]</button>
            </div>
            <div class="row">
              <select id="squad-list" class="term-input" title="Saved squads"></select>
              <button type="button" id="squad-load" class="term-btn" title="Load squad as selection">[LOAD]</button>
              <button type="button" id="squad-del" class="term-btn secondary" title="Delete squad">[x]</button>
            </div>
            <div class="row">
              <input id="squad-name" class="term-input" type="text" placeholder="squad name" />
              <button type="button" id="squad-save" class="term-btn" title="Save current selection as squad">[SAVE]</button>
            </div>
            <div class="row">
              <select id="sel-preset" class="term-input"></select>
              <button type="button" id="sel-preset-del" class="term-btn secondary" title="Delete preset">[x]</button>
            </div>
            <textarea id="sel-text" class="term-input" rows="4"
              placeholder="Type a prompt to send to selected agents..."></textarea>
            <div class="row">
              <input id="sel-preset-name" class="term-input" type="text" placeholder="preset name" />
              <button type="button" id="sel-preset-save" class="term-btn">[SAVE]</button>
            </div>
            <div class="row">
              <button type="button" id="sel-send" class="term-btn">[ SEND ]</button>
              <button type="button" id="op-inbox" class="term-btn secondary" title="Read operator inbox (dev token)">[ OP INBOX ]</button>
            </div>
            <div id="sel-delivery"></div>
          </div>
        </section>
        <section id="tab-inspect" class="tab-page">
          <!-- LEGACY popup content lands here in Task 7 -->
          <div id="inspect-popup" class="term-panel visible">
            <div class="title"><span id="inspect-title">INSPECT</span></div>
            <div id="inspect-body"></div>
          </div>
        </section>
      </aside>
    </main>

    <footer id="bottom">
      <div id="dock-log">
        <div id="log" aria-live="polite"></div>
      </div>
      <div id="help">
        KEYS: arrows move · g interact · LMB-drag box select · double-click all visible · MMB/RMB pan · right-click inspect · Ctrl/Shift add · SPACE follow · m mock
      </div>
    </footer>

    <script type="module" src="/static/app.js?v=__ASK_VER__"></script>
  </body>
</html>
```

`app.css` full content (complete rewrite — theme vars preserved from the old file's `:root` block; copy the var definitions from old app.css lines 1-40 verbatim into the new file's `:root`):

```css
/* ASK viewer — dock-grid shell + terminal primitives.
 * No fixed positioning anywhere: layout is one CSS grid; panels flow
 * inside their dock cells. */

/* :root { … copy the existing var(--*) block verbatim from old app.css … } */

* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body {
  display: grid;
  grid-template-rows: auto 1fr auto;
  height: 100vh;
  background: var(--bg, #0a0a0c);
  color: var(--r80-text, #c8d0d8);
  font-family: "SF Mono", ui-monospace, Menlo, Consolas, monospace;
  font-size: 13px;
  overflow: hidden;
}

/* --- hud --- */
#hud {
  display: flex; align-items: center; gap: var(--space-sm, 8px);
  padding: 4px var(--space-sm, 8px);
  border-bottom: 1px solid var(--r80-text, #c8d0d8);
  white-space: nowrap; overflow: hidden;
}
#hud .spacer { flex: 1; }
#status.online { color: var(--online, #00ff00); }
#status.offline { color: var(--offline, #ff5555); }
#mode.mode-mock { color: var(--r80-accent, #ffcc00); }
#mode.mode-human { color: var(--online, #00ff00); }
#info, #cam { opacity: 0.8; }
#theme-bar { display: flex; align-items: center; gap: var(--space-xs, 4px); }
#theme-bar label { opacity: 0.7; }

/* --- main grid --- */
#main {
  display: grid;
  grid-template-columns: auto 1fr auto;
  min-height: 0;
}
.dock {
  width: 300px; max-width: 42vw;
  overflow-y: auto; padding: var(--space-sm, 8px);
  display: flex; flex-direction: column; gap: var(--space-sm, 8px);
}
#dock-track { border-right: 1px solid var(--r80-neutral, #2a2d34); }
#dock-command { border-left: 1px solid var(--r80-neutral, #2a2d34); padding: 0; }
body.l-collapsed #dock-track,
body.r-collapsed #dock-command { display: none; }

/* --- viewport --- */
#viewport { position: relative; overflow: hidden; min-width: 0; }
#map { position: absolute; inset: 0; }
#select-box {
  position: absolute; display: none; z-index: 3;
  border: 1px dashed var(--r80-accent, #ffcc00);
  background: rgba(255, 204, 0, 0.08); pointer-events: none;
}
#select-box.active { display: block; }

/* --- command tabs --- */
#command-tabs { display: flex; border-bottom: 1px solid var(--r80-neutral, #2a2d34); }
.tab-btn {
  flex: 1; background: none; border: none; color: inherit;
  font: inherit; padding: 6px 0; cursor: pointer; opacity: 0.6;
  border-bottom: 2px solid transparent;
}
.tab-btn.active { opacity: 1; border-bottom-color: var(--r80-accent, #ffcc00); }
.tab-page { display: none; padding: var(--space-sm, 8px); overflow-y: auto; }
.tab-page.active { display: block; }

/* --- bottom --- */
#bottom {
  display: grid; grid-template-columns: 1fr auto;
  border-top: 1px solid var(--r80-text, #c8d0d8);
  min-height: 0;
}
#dock-log { overflow-y: auto; max-height: 20vh; padding: 2px var(--space-sm, 8px); }
#help { padding: 4px var(--space-sm, 8px); opacity: 0.6; align-self: end; }

/* --- terminal primitives (unchanged semantics) --- */
.term-btn {
  background: none; border: 1px solid var(--r80-text, #c8d0d8);
  color: inherit; font: inherit; padding: 1px 6px; cursor: pointer;
}
.term-btn:hover, .term-btn:focus { border-color: var(--r80-accent, #ffcc00); color: var(--r80-accent, #ffcc00); }
.term-btn.secondary { opacity: 0.7; }
.term-input {
  background: var(--r80-neutral, #14161a); border: 1px solid var(--r80-neutral, #2a2d34);
  color: inherit; font: inherit; padding: 2px 6px;
}
.title { color: var(--r80-accent, #ffcc00); font-weight: 700; letter-spacing: 0.06em; }
.muted { opacity: 0.6; font-size: 0.9em; }
.row { display: flex; gap: var(--space-xs, 4px); align-items: center; margin: 4px 0; }
.row .term-input { flex: 1; min-width: 0; }
textarea.term-input { resize: vertical; min-height: 60px; width: 100%; }

/* tracker items */
.track-item { border: 1px solid var(--r80-neutral, #2a2d34); padding: 4px 6px; margin: 4px 0; cursor: pointer; }
.track-item.active { border-color: var(--r80-accent, #ffcc00); }
.track-item .rm { float: right; background: none; border: none; color: inherit; cursor: pointer; font: inherit; }
.track-item .rm:hover { color: var(--offline, #ff5555); }
.track-item .meta, .track-item .tok { opacity: 0.6; font-size: 0.85em; }

/* selector chips + delivery */
#sel-chips { display: flex; flex-wrap: wrap; gap: 4px; margin: 4px 0; }
.sel-chip { border: 1px solid var(--r80-accent, #ffcc00); padding: 0 4px; font-size: 0.85em; white-space: nowrap; }
.sel-chip.out-of-fov { opacity: 0.45; border-style: dashed; }
.sel-chip .rm { background: none; border: none; color: inherit; cursor: pointer; font: inherit; padding: 0 0 0 2px; }
.sel-chip .rm:hover { color: var(--offline, #ff5555); }
#sel-delivery { font-size: 0.85em; max-height: 120px; overflow: auto; }
.delivery-row.delivery-fail { color: var(--offline, #ff5555); }
.delivery-row.delivery-read { color: var(--online, #00ff00); }
.delivery-row.delivery-pending { opacity: 0.7; }

/* inspect (docked — no popup chrome, always "visible" inside its tab) */
#inspect-popup { border: none; }
#inspect-body table { border-collapse: collapse; width: 100%; }
#inspect-body td { border: 1px solid var(--r80-neutral, #2a2d34); padding: 1px 6px; vertical-align: top; }
#inspect-body td:first-child { opacity: 0.7; white-space: nowrap; }

/* small screens: docks overlay the map instead of squeezing it */
@media (max-width: 1100px) {
  .dock {
    position: absolute; top: 0; bottom: 0; z-index: 6;
    background: var(--bg, #0a0a0c); max-width: 80vw;
  }
  #dock-track { left: 0; }
  #dock-command { right: 0; }
  #main { position: relative; }
}
```

- [ ] **Step 1: backup-check current behavior**

Run the server, open the page, screenshot/note current behavior (all panels work). This is the parity baseline.

- [ ] **Step 2: write the new index.html and app.css exactly as above**

Copy the `:root` variable block from the old `app.css` (everything inside `:root { … }`) into the new file before the `*` rule — themes depend on those vars.

- [ ] **Step 3: temporary CSS-compat check**

The old JS only touches element *content*, never positions — so the grid rewrite needs zero JS changes. One exception: `#inspect-popup` used `.visible` toggling; the new markup hardcodes `class="visible"` inside its tab, and old `hideInspectPopup` removing the class just hides content in the inspect tab (acceptable until Task 7).

- [ ] **Step 4: link-check + live smoke**

Run: `node scripts/check-frontend.mjs state.js render.js net.js input.js app.js`
Expected: `module graph links OK`

```bash
./target/debug/ask-kernel --serve --port 8090 &
```
Open the page: tracker left, dispatch right (DISPATCH tab), map center filling space, log bottom-left, help bottom-right, hud top with dock toggles (toggles not wired yet — Task 5). Resize below 1100px: docks overlay. All old interactions still work (track, select, send, inspect shows in INSPECT tab area).

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/static/index.html crates/ask-kernel/static/app.css
git commit -m "refactor(frontend): dock-grid shell replaces fixed-position scatter (T3)"
```

---

### Task 4: mapview.js — extract the map from render.js

**Files:**
- Create: `crates/ask-kernel/static/mapview.js`
- Modify: `crates/ask-kernel/static/render.js` (delete moved functions)
- Modify: `crates/ask-kernel/static/input.js` (import from mapview.js)
- Modify: `crates/ask-kernel/static/net.js` (import from mapview.js)

**Interfaces:**
- Consumes: `art.js` exports (T2), `themes.js` via `S.theme`, `state.js` S.
- Produces (all moved VERBATIM from render.js, only imports change): `focusAgent()`, `agentsInWorldRect(x0,y0,x1,y1)`, `visibleAgentIds()`, `updateSelectionHighlight()`, `cellSize()`, `worldAtScreen(clientX,clientY)`, `viewportAtScreen(clientX,clientY)`, `syncViewSize()`, `clampCamera()`, `centerOnTile(tx,ty)`, `zoomBy(delta,ax,ay)`, `dimColor(hex,factor)`, `drawSnap(snap)`, plus NEW `mountMapview(root)` and bus emissions `camera-changed`.

- [ ] **Step 1: create mapview.js with the move-table below**

Move these functions from `render.js` into `mapview.js` UNCHANGED (cut-paste, no edits to bodies):

```
focusAgent, agentsInWorldRect, visibleAgentIds, updateSelectionHighlight,
cellSize, worldAtScreen, viewportAtScreen, syncViewSize, clampCamera,
centerOnTile, zoomBy, dimColor, drawSnap
```

New header for mapview.js:

```js
/* ASK viewer — map: ROT display, camera, draw, coordinate queries.
 * Owns #map canvas + #select-box overlay. Emits camera-changed. */

import { S } from "./state.js";
import { emit } from "./bus.js";
import { decodeFeatIds, lookForFeat, lookForEntity, materialColor } from "./art.js";

/** Install the ROT display into the #map grid cell. */
export function mountMapview(root) {
  S.mapRoot = root; // syncViewSize appends the canvas here
}

// …moved functions follow, verbatim…
```

Adjustments to moved bodies (the ONLY allowed edits):
1. `syncViewSize` — where it does `el.map.appendChild(...)`, use `S.mapRoot.appendChild(...)`; where it reads `el.viewport.clientWidth/clientHeight`, use `document.getElementById("viewport")`.
2. `clampCamera`, `centerOnTile`, `zoomBy` — add `emit("camera-changed")` after camera mutation (one line each, at the end).
3. `drawSnap` — delete the `renderSelectionChips()` line (chips become dispatch-panel business, re-rendered on `snapshot` there).
4. Delete `updateHudCam` entirely — the hud panel (Task 5) renders cam text from `camera-changed`.

- [ ] **Step 2: slim render.js**

Delete the moved functions from `render.js`. What remains: `pushLog`, `formatEvents`, `renderTracker`, `renderPresets`, `updateSelectionPanel`, `renderSelectionChips`, `renderDelivery`, `applyThemeChrome`, `setupThemeSelect`, `updateModeHud`, `showInspectPopup`, `hideInspectPopup`, `renderEntityInspect`, `renderCellInspect` (these migrate to panels in Tasks 5-7). render.js's import of art.js helpers may shrink — re-check with grep.

- [ ] **Step 3: repoint input.js imports**

In `input.js` replace the `from "./render.js"` import block:

```js
import {
  drawSnap, syncViewSize, clampCamera, centerOnTile, focusAgent,
  worldAtScreen, viewportAtScreen, agentsInWorldRect, visibleAgentIds,
  cellSize, zoomBy,
} from "./mapview.js";
import { pushLog, updateSelectionPanel, updateSelectionHighlight } from "./render.js";
```

`hideInspectPopup` stays imported from render.js until Task 7. `updateHudCam` call sites (if any in input.js): delete them (hud owns cam text now).

- [ ] **Step 4: repoint net.js imports**

In `net.js`, move `focusAgent, syncViewSize, centerOnTile, drawSnap` from the render.js import to a new `from "./mapview.js"` import. `updateHudCam` call in net.js (if present): delete.

- [ ] **Step 5: link-check + live smoke**

Run: `node scripts/check-frontend.mjs state.js mapview.js render.js net.js input.js app.js`
Expected: `module graph links OK`

Live: page renders map, pan/zoom/box-select/follow all work (map behavior unchanged — pure move).

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/static/
git commit -m "refactor(frontend): extract mapview.js from render.js grab-bag (T4)"
```

---

### Task 5: panels/hud.js + panels/tracker.js (+ state.js sheds their el refs)

**Files:**
- Create: `crates/ask-kernel/static/panels/hud.js`
- Create: `crates/ask-kernel/static/panels/tracker.js`
- Modify: `crates/ask-kernel/static/state.js` (delete el refs for hud/tracker/theme-bar; add DOCK_KEY)
- Modify: `crates/ask-kernel/static/app.js` (mount panels, delete old wiring)
- Modify: `crates/ask-kernel/static/render.js` (delete renderTracker, applyThemeChrome, setupThemeSelect, updateModeHud)
- Modify: `crates/ask-kernel/static/net.js` (emit conn-status/hud-info/mode-changed/tracked-changed instead of touching el)

**Interfaces:**
- Consumes: bus.js (T1), mapview.js (T4).
- Produces: `mountHud(root)`, `mountTracker(root)`. Bus events: `conn-status {text, online}`, `hud-info string`, `mode-changed bool`, `camera-changed`, `theme-changed`, `tracked-changed`.

- [ ] **Step 1: panels/hud.js**

```js
/* HUD panel: connection status, mode, info line, cam readout, theme
 * select, FOLLOW/MOCK, dock collapse toggles. Owns #hud. */

import { S, saveTracked } from "../state.js";
import { on, emit, log } from "../bus.js";
import { THEMES, getTheme } from "../themes.js";

const DOCK_KEY = "ask-docks-v1";

function loadDocks() {
  try {
    return JSON.parse(localStorage.getItem(DOCK_KEY) || "{}");
  } catch (_) {
    return {};
  }
}

export function mountHud(root) {
  root.innerHTML = `
    <span id="status">…</span>
    <span id="mode" class="mode-mock">MOCK</span>
    <span id="info"></span>
    <span id="cam"></span>
    <span class="spacer"></span>
    <label for="theme">THEME</label>
    <select id="theme" aria-label="Map color theme"></select>
    <button type="button" id="btn-follow" class="term-btn" title="Follow focused agent">[ FOLLOW ]</button>
    <button type="button" id="btn-mock" class="term-btn secondary" title="Toggle mock">[ MOCK ]</button>
    <button type="button" id="dock-toggle-l" class="term-btn secondary" title="Toggle left dock">[ ◀ ]</button>
    <button type="button" id="dock-toggle-r" class="term-btn secondary" title="Toggle right dock">[ ▶ ]</button>`;

  const status = root.querySelector("#status");
  const mode = root.querySelector("#mode");
  const info = root.querySelector("#info");
  const cam = root.querySelector("#cam");
  const themeSel = root.querySelector("#theme");

  // theme select (moved from setupThemeSelect + applyThemeChrome)
  const applyChrome = () => {
    const u = S.theme.ui;
    const rs = document.documentElement.style;
    rs.setProperty("--bg", u.bg);
    rs.setProperty("--hud", u.hud);
    rs.setProperty("--hud-muted", u.hudMuted);
    rs.setProperty("--online", u.online);
    rs.setProperty("--offline", u.offline);
    document.body.style.background = u.bg;
    document.getElementById("viewport").style.background = u.bg;
  };
  for (const t of THEMES) {
    const opt = document.createElement("option");
    opt.value = t.id;
    opt.textContent = t.name;
    themeSel.appendChild(opt);
  }
  themeSel.value = S.theme.id;
  themeSel.addEventListener("change", () => {
    S.theme = getTheme(themeSel.value);
    localStorage.setItem("ask-theme", S.theme.id);
    applyChrome();
    emit("theme-changed");
    if (S.lastSnap) emit("snapshot", S.lastSnap); // force redraw
  });
  applyChrome();

  // dock collapse (persisted)
  const docks = loadDocks();
  document.body.classList.toggle("l-collapsed", !!docks.l);
  document.body.classList.toggle("r-collapsed", !!docks.r);
  const toggle = (side) => {
    const cls = side === "l" ? "l-collapsed" : "r-collapsed";
    document.body.classList.toggle(cls);
    const d = loadDocks();
    d[side] = document.body.classList.contains(cls);
    localStorage.setItem(DOCK_KEY, JSON.stringify(d));
    emit("camera-changed"); // viewport resized → reclamp
  };
  root.querySelector("#dock-toggle-l").addEventListener("click", () => toggle("l"));
  root.querySelector("#dock-toggle-r").addEventListener("click", () => toggle("r"));

  root.querySelector("#btn-follow").addEventListener("click", () => {
    S.cam.follow = true;
    log("FOLLOW ON");
    if (S.lastSnap) emit("snapshot", S.lastSnap);
  });
  root.querySelector("#btn-mock").addEventListener("click", async () => {
    const { setHumanControl } = await import("../net.js");
    setHumanControl(false);
    log("MOCK");
  });

  // bus subscriptions
  on("conn-status", ({ text, online }) => {
    status.textContent = text;
    status.className = online ? "online" : "offline";
  });
  on("hud-info", (text) => { info.textContent = text; });
  on("mode-changed", (human) => {
    mode.textContent = human ? "HUMAN" : "MOCK";
    mode.className = human ? "mode-human" : "mode-mock";
  });
  on("camera-changed", () => {
    cam.textContent = `cam(${S.cam.tx},${S.cam.ty}) z${S.cam.zi}`;
  });
}
```

- [ ] **Step 2: panels/tracker.js**

```js
/* Tracker panel: token input + tracked-agent list + follow-on-click.
 * Owns #dock-track. All data in S.tracked (localStorage-backed). */

import { S, saveTracked } from "../state.js";
import { on, emit, log } from "../bus.js";
import { centerOnTile } from "../mapview.js";
import { addToken, clearTracked } from "../net.js";

export function mountTracker(root) {
  root.innerHTML = `
    <div class="title">+-- AGENT TRACK --+</div>
    <div class="muted">paste token to spectate</div>
    <div class="row">
      <input id="token-input" class="term-input" type="text" spellcheck="false"
             autocomplete="off" placeholder="ask1_…" aria-label="Agent token" />
      <button type="button" id="token-add" class="term-btn" title="Add tracker">[+]</button>
      <button type="button" id="token-clear" class="term-btn secondary" title="Clear all tracked tokens">[ CLEAR ]</button>
    </div>
    <div id="tracker-list"></div>
    <div class="muted" id="tracker-hint">0 tracked (saved locally)</div>`;

  const input = root.querySelector("#token-input");
  const list = root.querySelector("#tracker-list");
  const hint = root.querySelector("#tracker-hint");

  root.querySelector("#token-add").addEventListener("click", () => {
    addToken(input.value);
    input.value = "";
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addToken(input.value);
      input.value = "";
    }
  });
  root.querySelector("#token-clear").addEventListener("click", () => clearTracked());

  const render = () => {
    list.innerHTML = "";
    S.tracked.forEach((t, i) => {
      const div = document.createElement("div");
      div.className = "track-item" + (S.followToken === t.token ? " active" : "");
      div.innerHTML =
        `<button type="button" class="rm" title="remove">[x]</button>` +
        `<div class="name" style="color:${t.color}">@ ${t.name || "agent"}</div>` +
        `<div class="meta">id=${t.agent_id ?? "?"}  @(${t.x ?? "?"},${t.y ?? "?"})</div>` +
        `<div class="tok">${t.token.slice(0, 18)}…</div>`;
      div.addEventListener("click", (e) => {
        if (e.target.classList.contains("rm")) return;
        S.followToken = t.token;
        S.cam.follow = true;
        emit("tracked-changed"); // net resubscribes with new focus
        if (t.x != null && t.y != null) {
          centerOnTile(t.x, t.y);
          if (S.lastSnap) emit("snapshot", S.lastSnap);
        }
        log(`FOLLOW ${t.name || t.token.slice(0, 12)}`);
      });
      div.querySelector(".rm").addEventListener("click", (e) => {
        e.stopPropagation();
        S.tracked.splice(i, 1);
        if (S.followToken === t.token)
          S.followToken = S.tracked.length ? S.tracked[0].token : null;
        saveTracked();
        emit("tracked-changed");
      });
      list.appendChild(div);
    });
    hint.textContent = `${S.tracked.length} tracked (saved locally)`;
  };
  on("tracked-changed", render);
  render();
}
```

- [ ] **Step 3: net.js emits instead of touching el**

In `net.js`:
1. Delete `renderTracker, updateModeHud` from its render.js import; add `import { emit } from "./bus.js";`.
2. `refreshTracked` — replace `renderTracker()` calls with `emit("tracked-changed")`.
3. `addToken` / `clearTracked` — replace `renderTracker()` with `emit("tracked-changed")`; replace `sendSubscribe()` calls with `emit("tracked-changed")` too (one event, two listeners: tracker re-renders, net resubscribes). Add at module level: `on("tracked-changed", () => sendSubscribe());` (import `on` too).
4. `applySnapshot` — replace the `el.info.textContent = …` block: build the same string, then `emit("hud-info", str)`. Replace `formatEvents(...)` call with `emit("events", snap.recent_events.filter((e) => e.type !== "tick_started"))`.
5. `connect` — replace `el.status.textContent/className` writes:
   - connecting: `emit("conn-status", { text: "connecting", online: false })`
   - live: `emit("conn-status", { text: "live", online: true })`
   - offline: `emit("conn-status", { text: "offline", online: false })`
6. `setHumanControl` — after setting `S.humanControl`, add `emit("mode-changed", S.humanControl)`; delete `updateModeHud()` call.
7. `applySnapshot` — this is the moment the map switches from direct calls to the bus. After `S.lastSnap = snap` and the tracked-pose sync, DELETE the direct `syncViewSize()/centerOnTile()/drawSnap()` calls and the whole `if (S.cam.follow)` block; end the function with `emit("snapshot", snap)`. Keep the `focusAgent` import (hud-info still needs it); drop `syncViewSize, centerOnTile, drawSnap` from net.js's mapview import.
8. In `mapview.js`, inside `mountMapview` (same commit, so the map never goes dark), subscribe the draw pipeline:

```js
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
```

After this commit net.js never imports draw functions — the wire layer only emits.

- [ ] **Step 4: app.js mounts the two panels**

In `app.js`:
1. Add imports: `import { mountHud } from "./panels/hud.js";` and `import { mountTracker } from "./panels/tracker.js";`.
2. Replace the token-panel wiring block (tokenAdd/tokenClear/tokenInput listeners), the btnFollow block, the btnMock block, and `setupThemeSelect(); updateModeHud(); renderTracker();` boot calls with:

```js
mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
```

3. Delete from index.html: the old `<div id="theme-bar">…</div>` markup (hud.js renders those controls now — the ids live in the hud template) and the legacy `<div id="tracker">…</div>` wrapper inside `#dock-track` (tracker.js renders it). Keep the outer `<header id="hud">` and `<aside id="dock-track" class="dock">` EMPTY as mount points.

- [ ] **Step 5: state.js sheds refs**

Delete from `el` in state.js: `status, info, cam, theme, mode, btnFollow, btnMock, tokenInput, tokenAdd, tokenClear, trackerList, trackerHint`. Delete from render.js: `renderTracker, applyThemeChrome, setupThemeSelect, updateModeHud` (and now-unused imports).

- [ ] **Step 6: link-check + live smoke**

Run: `node scripts/check-frontend.mjs state.js mapview.js render.js net.js input.js panels/hud.js panels/tracker.js app.js`
Expected: `module graph links OK`

Live: hud shows status/mode/info/cam; theme select changes theme + redraws; FOLLOW/MOCK work; ◀ ▶ collapse docks and survive reload; tracker add/remove/follow works; follow click centers map.

- [ ] **Step 7: Commit**

```bash
git add crates/ask-kernel/static/
git commit -m "refactor(frontend): hud + tracker become self-contained panels (T5)"
```

---

### Task 6: panels/dispatch.js (+ selection mutators into state.js)

**Files:**
- Create: `crates/ask-kernel/static/panels/dispatch.js`
- Modify: `crates/ask-kernel/static/state.js` (add selection mutators; delete sel-*/squad-*/op-inbox el refs)
- Modify: `crates/ask-kernel/static/net.js` (sendPromptToSelected/fetchOperatorInbox → raw API fns; delivery moves to dispatch)
- Modify: `crates/ask-kernel/static/render.js` (delete renderPresets, updateSelectionPanel, renderSelectionChips, renderDelivery, updateSelectionHighlight)
- Modify: `crates/ask-kernel/static/input.js` (selection mutators from state.js)
- Modify: `crates/ask-kernel/static/app.js` (mount dispatch, delete squad/preset wiring)
- Create: `tests/frontend/state.test.mjs`

**Interfaces:**
- Consumes: bus (T1), mapview `visibleAgentIds` (T4).
- Produces: `mountDispatch(root)`; state.js `setSelectedAgents(ids)`, `addSelectedAgents(ids)`, `toggleSelectAgent(id)` (each emits `selection-changed`); net.js `apiSendMessage(token, targets, text)`, `apiMessageStatus(token, ids)`, `apiOperatorInbox(token)` — raw fetch wrappers returning parsed JSON.

- [ ] **Step 1: failing test for selection mutators**

```js
// tests/frontend/state.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";

// state.js touches localStorage at module scope — stub before import
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
globalThis.location = { protocol: "http:", host: "x" };

const { S, setSelectedAgents, addSelectedAgents, toggleSelectAgent } =
  await import("../../crates/ask-kernel/static/state.js");
const { on } = await import("../../crates/ask-kernel/static/bus.js");

test("selection mutators update the set and emit", () => {
  let fires = 0;
  on("selection-changed", () => fires++);
  setSelectedAgents([1, 2]);
  assert.deepEqual([...S.selectedAgentIds].sort(), [1, 2]);
  addSelectedAgents([3]);
  toggleSelectAgent(1); // remove
  toggleSelectAgent(4); // add
  assert.deepEqual([...S.selectedAgentIds].sort(), [2, 3, 4]);
  assert.equal(fires, 4);
});
```

Run: `node --test tests/frontend/` — Expected: FAIL (`setSelectedAgents is not exported`).

- [ ] **Step 2: state.js mutators**

Add to `state.js` (and delete the same functions' future need in input.js):

```js
import { emit } from "./bus.js";

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
```

Delete from `el`: `selCount, selAllVis, selClear, selPreset, selPresetDel, selPresetSave, selPresetName, selText, selSend, selChips, selDelivery, squadList, squadLoad, squadSave, squadDel, squadName, opInbox`.

Run: `node --test tests/frontend/` — Expected: PASS.

- [ ] **Step 3: net.js raw message API**

Replace `sendPromptToSelected` and `fetchOperatorInbox` with three raw wrappers (no UI concerns — dispatch owns S.delivery and polling):

```js
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
```

Also change `fetchEntityInspect`/`fetchCellInspect` to emit instead of rendering: replace their `renderEntityInspect(d.entity)` / `renderCellInspect(d.cell)` calls with `emit("inspect-show", { kind: "entity", data: d.entity })` / `emit("inspect-show", { kind: "cell", data: d.cell })` (Task 7's panel renders). Delete their render.js imports for those two functions. And subscribe at module level:

```js
on("request-inspect-entity", (id) => fetchEntityInspect(id));
on("request-inspect-cell", ({ x, y }) => fetchCellInspect(x, y));
```

- [ ] **Step 4: panels/dispatch.js (complete)**

```js
/* Dispatch panel: selection chips, squads, prompt presets, SEND with
 * per-target delivery receipts, operator inbox. Owns #tab-dispatch.
 * Squads/presets live in localStorage — the server never knows. */

import {
  S, inspectToken, agentName,
  loadPresets, savePresets, loadSquads, saveSquads,
  setSelectedAgents,
} from "../state.js";
import { on, log } from "../bus.js";
import { visibleAgentIds } from "../mapview.js";
import { apiSendMessage, apiMessageStatus, apiOperatorInbox } from "../net.js";

export function mountDispatch(root) {
  root.innerHTML = `
    <div class="title">+ SELECTOR +</div>
    <div class="muted" id="sel-count">0 agents selected</div>
    <div class="muted">LMB drag box · double-click all visible · MMB/RMB pan</div>
    <div id="sel-chips"></div>
    <div class="row">
      <button type="button" id="sel-all-vis" class="term-btn" title="Select all agents currently in FOV">[ ALL VIS ]</button>
      <button type="button" id="sel-clear" class="term-btn secondary" title="Clear selection">[ CLEAR ]</button>
    </div>
    <div class="row">
      <select id="squad-list" class="term-input" title="Saved squads"></select>
      <button type="button" id="squad-load" class="term-btn" title="Load squad">[LOAD]</button>
      <button type="button" id="squad-del" class="term-btn secondary" title="Delete squad">[x]</button>
    </div>
    <div class="row">
      <input id="squad-name" class="term-input" type="text" placeholder="squad name" />
      <button type="button" id="squad-save" class="term-btn" title="Save selection as squad">[SAVE]</button>
    </div>
    <div class="row">
      <select id="sel-preset" class="term-input"></select>
      <button type="button" id="sel-preset-del" class="term-btn secondary" title="Delete preset">[x]</button>
    </div>
    <textarea id="sel-text" class="term-input" rows="4"
      placeholder="Type a prompt to send to selected agents..."></textarea>
    <div class="row">
      <input id="sel-preset-name" class="term-input" type="text" placeholder="preset name" />
      <button type="button" id="sel-preset-save" class="term-btn">[SAVE]</button>
    </div>
    <div class="row">
      <button type="button" id="sel-send" class="term-btn">[ SEND ]</button>
      <button type="button" id="op-inbox" class="term-btn secondary" title="Read operator inbox (dev token)">[ OP INBOX ]</button>
    </div>
    <div id="sel-delivery"></div>`;

  const $ = (id) => root.querySelector("#" + id);
  const chips = $("sel-chips"), delivery = $("sel-delivery"),
        count = $("sel-count"), text = $("sel-text"),
        presetSel = $("sel-preset"), presetName = $("sel-preset-name"),
        squadSel = $("squad-list"), squadName = $("squad-name");

  // ---- chips (re-rendered on selection change + snapshot for FOV dim) ----
  const renderChips = () => {
    count.textContent = `${S.selectedAgentIds.size} agents selected`;
    chips.innerHTML = "";
    const visible = new Set(visibleAgentIds());
    for (const id of S.selectedAgentIds) {
      const chip = document.createElement("span");
      chip.className = "sel-chip" + (visible.has(id) ? "" : " out-of-fov");
      chip.title = visible.has(id) ? "in FOV" : "out of FOV — send will be rejected";
      chip.textContent = agentName(id) + " ";
      const rm = document.createElement("button");
      rm.type = "button"; rm.className = "rm"; rm.textContent = "[x]";
      rm.addEventListener("click", (e) => {
        e.stopPropagation();
        S.selectedAgentIds.delete(id);
        renderChips(); // local re-render; setSelectedAgents not needed
      });
      chip.appendChild(rm);
      chips.appendChild(chip);
    }
  };
  on("selection-changed", renderChips);
  on("snapshot", renderChips); // FOV dimming follows the live map

  $("sel-all-vis").addEventListener("click", () => {
    const ids = visibleAgentIds();
    setSelectedAgents(ids);
    log(`SELECTED ${ids.length} visible agents`);
  });
  $("sel-clear").addEventListener("click", () => {
    setSelectedAgents([]);
    log("CLEARED selection");
  });

  // ---- squads (localStorage selection sets) ----
  const renderSquads = () => {
    const squads = loadSquads();
    squadSel.innerHTML = "";
    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = squads.length ? "(squads)" : "(no squads)";
    squadSel.appendChild(empty);
    for (const sq of squads) {
      const opt = document.createElement("option");
      opt.value = sq.name;
      opt.textContent = `${sq.name} (${sq.ids.length})`;
      squadSel.appendChild(opt);
    }
  };
  $("squad-save").addEventListener("click", () => {
    const name = (squadName.value || "").trim();
    if (!name) return log("SQUAD: name it first");
    if (!S.selectedAgentIds.size) return log("SQUAD: empty selection");
    const squads = loadSquads().filter((s) => s.name !== name);
    squads.push({ name, ids: [...S.selectedAgentIds] });
    saveSquads(squads);
    squadName.value = "";
    renderSquads();
    log(`SQUAD saved "${name}" (${S.selectedAgentIds.size})`);
  });
  $("squad-load").addEventListener("click", () => {
    const sq = loadSquads().find((s) => s.name === squadSel.value);
    if (!sq) return;
    setSelectedAgents(sq.ids);
    log(`SQUAD "${sq.name}" → ${sq.ids.length} selected`);
  });
  $("squad-del").addEventListener("click", () => {
    const name = squadSel.value;
    if (!name) return;
    saveSquads(loadSquads().filter((s) => s.name !== name));
    renderSquads();
    log(`SQUAD deleted "${name}"`);
  });
  renderSquads();

  // ---- presets (localStorage prompt templates) ----
  const renderPresets = () => {
    const presets = loadPresets();
    presetSel.innerHTML = "";
    const none = document.createElement("option");
    none.value = "";
    none.textContent = presets.length ? "(presets)" : "(no presets)";
    presetSel.appendChild(none);
    for (const p of presets) {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.name;
      presetSel.appendChild(opt);
    }
  };
  presetSel.addEventListener("change", () => {
    const p = loadPresets().find((x) => x.id === presetSel.value);
    if (p) text.value = p.text;
  });
  $("sel-preset-save").addEventListener("click", () => {
    const name = (presetName.value || "").trim();
    if (!name) return log("PRESET: name it first");
    if (!text.value.trim()) return log("PRESET: empty text");
    const presets = loadPresets();
    presets.push({ id: `p${Date.now()}`, name, text: text.value });
    savePresets(presets);
    presetName.value = "";
    renderPresets();
    log(`PRESET saved "${name}"`);
  });
  $("sel-preset-del").addEventListener("click", () => {
    const id = presetSel.value;
    if (!id) return;
    savePresets(loadPresets().filter((p) => p.id !== id));
    renderPresets();
    text.value = "";
    log("PRESET deleted");
  });
  renderPresets();

  // ---- delivery receipts ----
  const renderDelivery = () => {
    delivery.innerHTML = "";
    for (const d of S.delivery) {
      const row = document.createElement("div");
      if (!d.ok) {
        row.className = "delivery-row delivery-fail";
        row.textContent = `✗ ${agentName(d.agent)} — ${d.reason}`;
      } else if (d.read_tick != null) {
        row.className = "delivery-row delivery-read";
        row.textContent = `✓ ${agentName(d.agent)} — read (tick ${d.read_tick})`;
      } else {
        row.className = "delivery-row delivery-pending";
        row.textContent = `… ${agentName(d.agent)} — unread`;
      }
      delivery.appendChild(row);
    }
  };
  const pollStatus = async () => {
    const pending = () => S.delivery.filter((d) => d.ok && d.read_tick == null);
    for (let i = 0; i < 15 && pending().length; i++) {
      await new Promise((r) => setTimeout(r, 2000));
      const token = inspectToken();
      if (!token) return;
      try {
        const d = await apiMessageStatus(token, pending().map((x) => x.msg_id));
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
  };
  $("sel-send").addEventListener("click", async () => {
    const token = inspectToken();
    if (!token) return log("SEND: track a token first");
    if (!S.selectedAgentIds.size) return log("SEND: select agents first");
    if (!text.value.trim()) return log("SEND: empty prompt");
    if (text.value.length > 500) return log("SEND: prompt too long (>500)");
    try {
      const d = await apiSendMessage(token, [...S.selectedAgentIds], text.value);
      if (!d.ok) return log("SEND: " + (d.reason || "failed"));
      S.delivery = (d.results || []).map((x) => ({
        agent: x.id, msg_id: x.msg_id, ok: !!x.ok, reason: x.reason, read_tick: null,
      }));
      renderDelivery();
      log(`SEND → ${d.sent} agents, ${d.rejected} rejected`);
      pollStatus();
    } catch (_) {
      log("SEND: network");
    }
  });
  $("op-inbox").addEventListener("click", async () => {
    const token = inspectToken();
    if (!token) return log("INBOX: track a token first");
    try {
      const d = await apiOperatorInbox(token);
      if (!d.ok) return log("INBOX: " + (d.reason || "failed"));
      if (!d.messages.length) return log("INBOX: empty");
      for (const m of d.messages) log(`◀ ${m.from} (tick ${m.sent_tick}): ${m.text}`);
    } catch (_) {
      log("INBOX: network");
    }
  });

  renderChips();
}
```

- [ ] **Step 5: rewire input.js selection**

In `input.js`: delete its local `setSelectedAgents/addSelectedAgents/toggleSelectAgent` definitions; import them from `./state.js` instead. Its `selectAllVisibleAgents` keeps calling `setSelectedAgents(visibleAgentIds())` (already imports visibleAgentIds from mapview). Delete `updateSelectionPanel`/`updateSelectionHighlight` imports — chips re-render via `selection-changed`, map highlight via mapview's own subscription. Add to mapview.js (T4 file):

```js
// in mapview.js, after mountMapview:
on("selection-changed", () => {
  if (S.lastSnap) drawSnap(S.lastSnap);
});
```

(put this `on(...)` call inside `mountMapview` so it subscribes once at boot; `updateSelectionHighlight` export may then be deleted.)

- [ ] **Step 6: app.js + index.html + render.js cleanup**

- app.js: add `import { mountDispatch } from "./panels/dispatch.js";` and `mountDispatch(document.getElementById("tab-dispatch"));`. Delete the entire preset + squad wiring blocks (now in dispatch.js).
- index.html: replace the legacy `<div id="selector-panel">…</div>` content inside `#tab-dispatch` with nothing (dispatch.js renders its own template); the section becomes `<section id="tab-dispatch" class="tab-page active"></section>`.
- render.js: delete `renderPresets, updateSelectionPanel, renderSelectionChips, renderDelivery, updateSelectionHighlight` (and now-unused imports like loadPresets, agentName).
- mapview.js `drawSnap`: confirm the `renderSelectionChips()` line is gone (T4 step) — chips render on `snapshot` in dispatch.

- [ ] **Step 7: tests + link-check + live smoke**

Run: `node --test tests/frontend/` — Expected: PASS (bus + state).
Run: `node scripts/check-frontend.mjs state.js mapview.js render.js net.js input.js panels/hud.js panels/tracker.js panels/dispatch.js app.js`
Expected: `module graph links OK`

Live: box-select → chips appear with names; walk agent out of FOV → chip dims; save/load squad; save/apply preset; SEND → delivery rows appear, unread → read ticks fill in (watch an agent view); OP INBOX with dev token shows agent replies.

- [ ] **Step 8: Commit**

```bash
git add crates/ask-kernel/static/ tests/frontend/
git commit -m "refactor(frontend): dispatch panel owns selector; selection mutators into state (T6)"
```

---

### Task 7: panels/inspect.js + panels/logview.js + tab controller

**Files:**
- Create: `crates/ask-kernel/static/panels/inspect.js`
- Create: `crates/ask-kernel/static/panels/logview.js`
- Modify: `crates/ask-kernel/static/app.js` (mount both + tab switching)
- Modify: `crates/ask-kernel/static/render.js` (DELETE the file if empty after this)
- Modify: `crates/ask-kernel/static/state.js` (delete log/inspect-* el refs)
- Modify: `crates/ask-kernel/static/index.html` (delete legacy #log/#help/#inspect-popup markup)

**Interfaces:**
- Consumes: net.js `inspect-show` emissions (T6 step 3), bus `log`/`events` (T5).
- Produces: `mountInspect(root)`, `mountLogview(logRoot, helpRoot)`; bus event `activate-tab`.

- [ ] **Step 1: panels/inspect.js (complete)**

```js
/* Inspect panel: docked entity/cell details in the right-dock tab.
 * Replaces the floating popup — details stay open while you watch. */

import { on } from "../bus.js";

function renderKV(obj, skip = []) {
  const rows = [];
  for (const [k, v] of Object.entries(obj)) {
    if (skip.includes(k) || v === null || v === undefined) continue;
    let display = v;
    if (Array.isArray(v)) {
      display = v
        .map((it) =>
          typeof it === "object"
            ? Object.entries(it).map(([kk, vv]) => `${kk}:${vv}`).join(" ")
            : String(it),
        )
        .join("<br>");
    } else if (typeof v === "object") {
      display = renderKV(v);
    } else {
      display = String(v);
    }
    rows.push(`<tr><td>${k}</td><td>${display}</td></tr>`);
  }
  return `<table>${rows.join("")}</table>`;
}

export function mountInspect(root) {
  root.innerHTML = `
    <div class="title"><span id="inspect-title">INSPECT</span></div>
    <div class="muted">right-click the map — entity or cell details dock here</div>
    <div id="inspect-body"></div>`;
  const title = root.querySelector("#inspect-title");
  const body = root.querySelector("#inspect-body");

  on("inspect-show", ({ kind, data }) => {
    if (kind === "entity") {
      title.textContent = `[${data.glyph || "?"}] ${data.kind || "unknown"}${data.name ? " — " + data.name : ""}`;
      body.innerHTML =
        `<div class="muted">position (${data.x}, ${data.y}) · id ${data.id}</div>` +
        renderKV(data, ["id", "x", "y", "glyph"]);
    } else {
      title.textContent = `[${data.glyph || " "}] ${data.name || "cell"}`;
      body.innerHTML =
        `<div class="muted">position (${data.x}, ${data.y}) · feat ${data.feat_id}</div>` +
        renderKV(data, ["x", "y", "glyph", "name", "feat_id"]);
    }
    // switch the right dock to this tab so the details are visible
    document.dispatchEvent(new CustomEvent("ask-activate-inspect"));
  });
}
```

- [ ] **Step 2: panels/logview.js (complete)**

Move `pushLog`'s list logic and `formatEvents` VERBATIM from render.js, reshaped as a bus consumer:

```js
/* Log panel: event feed + help line. Owns #dock-log and #help. */

import { on } from "../bus.js";

const HELP_TEXT =
  "KEYS: arrows move · g interact · LMB-drag box select · double-click all visible · MMB/RMB pan · right-click inspect · Ctrl/Shift add · SPACE follow · m mock";

export function mountLogview(logRoot, helpRoot) {
  if (helpRoot) helpRoot.textContent = HELP_TEXT;

  const push = (msg) => {
    const line = document.createElement("div");
    line.textContent = "> " + msg;
    logRoot.insertBefore(line, logRoot.firstChild);
    while (logRoot.querySelectorAll("div").length > 10) {
      const nodes = logRoot.querySelectorAll("div");
      logRoot.removeChild(nodes[nodes.length - 1]);
    }
  };
  on("log", push);
  on("events", (events) => {
    for (const ev of (events || []).slice(-4)) {
      const t = ev.type || "?";
      if (t === "moved") push(`→ (${ev.to[0]},${ev.to[1]})`);
      else if (t === "move_failed") push(`✗ ${ev.reason}`);
      else if (t === "harvested") push(`✂ ${ev.kind} +${ev.amount}`);
      else if (t === "built") push(`⌂ hut @(${ev.at[0]},${ev.at[1]})`);
      else if (t === "trap_triggered") push(`⚠ trap ${ev.name} -${ev.damage}`);
      else if (t === "terrain_damage") push(`♨ ${ev.kind} -${ev.damage} hp=${ev.hp}`);
      else if (t === "door_opened") push(`开门 (${ev.at[0]},${ev.at[1]})`);
      else if (t === "door_closed") push(`关门 (${ev.at[0]},${ev.at[1]})`);
      else if (t === "level_changed") push(`↕ depth=${ev.depth}`);
      else if (t === "item_picked_up") push(`拾取 ${ev.name}`);
      else if (t === "item_dropped") push(`丢下 ${ev.name}`);
      else if (t === "monster_attacked") push(`⚔ ${ev.name} -${ev.damage} hp=${ev.target_hp}`);
      else if (t === "player_attacked") push(`击中 ${ev.name} -${ev.damage} hp=${ev.target_hp}`);
      else if (t === "monster_killed") push(`击杀 ${ev.name}`);
      else if (t === "dug") push(`挖 (${ev.at[0]},${ev.at[1]}) → pack`);
      else if (t === "scooped") push(`铲 (${ev.at[0]},${ev.at[1]}) → pack`);
      else if (t === "placed") push(`放 feat=${ev.feat} @(${ev.at[0]},${ev.at[1]})`);
      else if (t === "crafted") push(`合成 ${ev.recipe}`);
      else if (t === "planted") push(`种植 @(${ev.at[0]},${ev.at[1]})`);
      else if (t === "deconstructed") push(`拆 hut +${ev.wood} wood`);
      else if (t === "rested") push(`休 +${ev.healed} hp=${ev.hp}`);
      else if (t === "agent_died") push(`☠ 倒下 @(${ev.at[0]},${ev.at[1]})`);
      else if (t === "agent_respawned") push(`✚ 重生 @(${ev.at[0]},${ev.at[1]})`);
      else if (t === "terrain_changed") push(`≋ ${ev.cause} (${ev.at[0]},${ev.at[1]})`);
      else if (t === "consumed") push(`吃 ${ev.label} hp=${ev.hp}`);
    }
  });
}
```

- [ ] **Step 3: tab controller in app.js**

In `app.js` add (after the other mounts):

```js
import { mountInspect } from "./panels/inspect.js";
import { mountLogview } from "./panels/logview.js";
import { on } from "./bus.js";

mountInspect(document.getElementById("tab-inspect"));
mountLogview(document.getElementById("log"), document.getElementById("help"));

// right-dock tabs: DISPATCH | INSPECT
const tabs = document.getElementById("command-tabs");
const activate = (name) => {
  for (const b of tabs.querySelectorAll(".tab-btn"))
    b.classList.toggle("active", b.dataset.tab === name);
  document.getElementById("tab-dispatch").classList.toggle("active", name === "dispatch");
  document.getElementById("tab-inspect").classList.toggle("active", name === "inspect");
};
tabs.addEventListener("click", (e) => {
  if (e.target.dataset.tab) activate(e.target.dataset.tab);
});
document.addEventListener("ask-activate-inspect", () => activate("inspect"));
on("activate-tab", activate);
```

- [ ] **Step 4: delete the last render.js remnants + legacy markup**

- render.js: after T5/T6/T7 deletions only `pushLog`/`formatEvents`/`showInspectPopup`/`hideInspectPopup`/`renderEntityInspect`/`renderCellInspect`/`renderKV` remain — all now dead. Delete `render.js`. Delete its import from any remaining file (`grep -rn "render.js" static/` must return nothing).
- index.html: delete the legacy `<div id="inspect-popup">…</div>` content inside `#tab-inspect` (inspect.js renders its own) and the inner `<div id="log">` content stays as the mount point `<div id="log" aria-live="polite"></div>`; keep `<div id="help"></div>` empty.
- state.js: delete `log, inspectPopup, inspectTitle, inspectBody, inspectClose` from `el`; also `selectBox, viewport, map` (mapview/input query by id now — remove from el and update any user).
- input.js: `hideInspectPopup` import/calls → replace with nothing (inspect is docked; Escape emits `activate-tab:dispatch` via `emit("activate-tab", "dispatch")`). `inspectClose` listener in app.js → delete. Right-click inspect calls in input.js: replace `fetchEntityInspect(id)`/`fetchCellInspect(...)` direct imports with `emit("request-inspect-entity", id)` / `emit("request-inspect-cell", {x, y})`.

- [ ] **Step 5: link-check + live smoke**

Run: `grep -rn "render\.js" crates/ask-kernel/static/ || echo "render.js fully retired"`
Run: `node scripts/check-frontend.mjs state.js mapview.js net.js input.js panels/hud.js panels/tracker.js panels/dispatch.js panels/inspect.js panels/logview.js app.js`
Expected: `module graph links OK`

Live: right-click an agent → INSPECT tab auto-activates with entity KV; right-click a wall → cell details; event log scrolls at bottom-left; Esc returns to DISPATCH tab.

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/static/
git commit -m "refactor(frontend): inspect docks into tab, log panel; render.js retired (T7)"
```

---

### Task 8: input.js final rewiring + state.js el registry deletion

**Files:**
- Modify: `crates/ask-kernel/static/input.js`
- Modify: `crates/ask-kernel/static/state.js` (delete the entire `el` object)
- Modify: `crates/ask-kernel/static/mapview.js`, `net.js`, `app.js` (any remaining el uses)

**Interfaces:**
- Consumes: everything above.
- Produces: state.js with zero DOM refs (Global Constraint).

- [ ] **Step 1: find remaining el users**

Run: `grep -rn "el\.\|{ el" crates/ask-kernel/static/*.js crates/ask-kernel/static/panels/*.js`
Expected: a short list — input.js (`selectBox`, `viewport`, `status`?), net.js leftovers, mapview leftovers. Every hit gets replaced with a local `document.getElementById(...)` in the owning module (viewport/select-box → mapview's domain; input.js queries them locally).

- [ ] **Step 2: input.js final import header**

```js
import { S, setSelectedAgents, addSelectedAgents, toggleSelectAgent } from "./state.js";
import { emit, log } from "./bus.js";
import {
  drawSnap, syncViewSize, clampCamera, centerOnTile, focusAgent,
  worldAtScreen, viewportAtScreen, agentsInWorldRect, visibleAgentIds,
  cellSize, zoomBy,
} from "./mapview.js";
import { sendAction, setHumanControl, applySnapshot } from "./net.js";
```

Replace all `pushLog(...)` with `log(...)`, `el.viewport`/`el.selectBox` with local `const viewport = document.getElementById("viewport")` / `selectBox = document.getElementById("select-box")` inside `installInputHandlers`. Verify keyboard handlers still fire (they attach to `window`).

- [ ] **Step 3: delete `el` from state.js**

Remove the whole `export const el = { … }` block. `grep -rn "\bel\b" static/ | grep -v panels` must show no state.js `el` imports.

- [ ] **Step 4: full verification**

Run: `node --test tests/frontend/`
Run: `node scripts/check-frontend.mjs state.js bus.js mapview.js net.js input.js panels/hud.js panels/tracker.js panels/dispatch.js panels/inspect.js panels/logview.js app.js`
Live: full interaction pass — track/follow/theme/mock/collapse; box-select/squads/presets/send/delivery/op-inbox; inspect tabs; keyboard (arrows, g, o/c/f/t direction prompts, space, m); pan/zoom; log scrolling.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/static/
git commit -m "refactor(frontend): state.js sheds el registry — zero DOM in shared state (T8)"
```

---

### Task 9: app.js final boot + verification + docs

**Files:**
- Modify: `crates/ask-kernel/static/app.js` (final form)
- Modify: `crates/ask-kernel/README.md` or `docs/ARCHITECTURE.md` (frontend section)

**Interfaces:**
- Consumes: all.

- [ ] **Step 1: final app.js**

```js
/* ASK viewer — boot: mount panels into the grid shell, install input,
 * connect. Panels own their DOM; cross-talk via bus.js only. */

import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";
import { mountDispatch } from "./panels/dispatch.js";
import { mountInspect } from "./panels/inspect.js";
import { mountLogview } from "./panels/logview.js";
import { mountMapview } from "./mapview.js";
import { installInputHandlers } from "./input.js";
import { on } from "./bus.js";
import { connect, refreshTracked } from "./net.js";

mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
mountMapview(document.getElementById("map"));
mountDispatch(document.getElementById("tab-dispatch"));
mountInspect(document.getElementById("tab-inspect"));
mountLogview(document.getElementById("log"), document.getElementById("help"));

// right-dock tabs: DISPATCH | INSPECT
const tabs = document.getElementById("command-tabs");
const activate = (name) => {
  for (const b of tabs.querySelectorAll(".tab-btn"))
    b.classList.toggle("active", b.dataset.tab === name);
  document.getElementById("tab-dispatch").classList.toggle("active", name === "dispatch");
  document.getElementById("tab-inspect").classList.toggle("active", name === "inspect");
};
tabs.addEventListener("click", (e) => {
  if (e.target.dataset.tab) activate(e.target.dataset.tab);
});
document.addEventListener("ask-activate-inspect", () => activate("inspect"));
on("activate-tab", activate);

installInputHandlers();
refreshTracked();
connect();
```

- [ ] **Step 2: final link-check + unit tests + full live smoke**

```bash
node --test tests/frontend/
node scripts/check-frontend.mjs state.js bus.js mapview.js net.js input.js \
  panels/hud.js panels/tracker.js panels/dispatch.js panels/inspect.js \
  panels/logview.js app.js
cargo test -p ask-kernel 2>&1 | grep -E "FAILED|failures" || echo "kernel suite clean"
```

Live full pass (the T8 Step 4 checklist) against a fresh server with 2+ registered agents.

- [ ] **Step 3: document the frontend architecture**

Append to `docs/ARCHITECTURE.md`:

```markdown
## Frontend (static/)

Dock-grid shell (index.html = the DOM id contract) + self-contained panel
modules (`mount(root)`; own template/queries/listeners). Cross-module
traffic only via bus.js events (table in
docs/superpowers/plans/2026-07-18-frontend-dock-rearchitecture.md).
state.js holds S + storage + pure helpers — zero DOM refs. mapview owns
the ROT canvas; net owns the wire; input turns gestures into intents.
Verify with `node --test tests/frontend/` + `node scripts/check-frontend.mjs <modules>`.
```

- [ ] **Step 4: Commit**

```bash
git add crates/ask-kernel/static/ docs/ARCHITECTURE.md
git commit -m "refactor(frontend): final boot assembly + architecture doc (T9)"
```

---

## Self-Review Notes

- **Spec coverage:** diagnosis items 1→T3 (grid shell), 2→T5/T6/T7/T8 (el deletion per panel + final purge), 3→T1+T5 (bus breaks cycle), 4→T2, 5→T4-T7 (render.js fully retired), 6→T7 (docked inspect), 7→T3 (grid + collapse + media query). Every existing feature has a migration home (Global Constraints list ↔ panel tasks).
- **Type consistency:** bus event names match the table across tasks (`selection-changed`, `tracked-changed`, `snapshot`, `inspect-show`, `activate-tab`, `conn-status`, `hud-info`, `mode-changed`, `camera-changed`, `theme-changed`, `request-inspect-entity`, `request-inspect-cell`, `log`, `events`). Panel exports: `mountHud/mountTracker/mountDispatch/mountInspect/mountLogview/mountMapview` — used identically in app.js finals (T7 Step 3 list = T9 Step 1 list).
- **Sequencing:** legacy ids survive in the shell until their panel task lands (T3 keeps them; T5/T6/T7 each delete their own legacy markup), so every commit leaves a working page.
