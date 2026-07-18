/* ASK viewer — input: keyboard commands, RTS box-select, camera pan, inspect
 * clicks. All DOM event wiring installed via installInputHandlers(). */

import { el, S } from "./state.js";
import {
  drawSnap,
  syncViewSize,
  clampCamera,
  centerOnTile,
  focusAgent,
  worldAtScreen,
  viewportAtScreen,
  agentsInWorldRect,
  visibleAgentIds,
  cellSize,
  zoomBy,
  updateSelectionHighlight,
} from "./mapview.js";
import { pushLog, updateSelectionPanel, hideInspectPopup } from "./render.js";
import {
  sendAction,
  setHumanControl,
  applySnapshot,
  fetchEntityInspect,
  fetchCellInspect,
} from "./net.js";

// ---------------------------------------------------------------- selection

export function setSelectedAgents(ids) {
  S.selectedAgentIds = new Set(ids);
  updateSelectionPanel();
  updateSelectionHighlight();
}

export function addSelectedAgents(ids) {
  for (const id of ids) S.selectedAgentIds.add(id);
  updateSelectionPanel();
  updateSelectionHighlight();
}

export function toggleSelectAgent(id) {
  if (S.selectedAgentIds.has(id)) {
    S.selectedAgentIds.delete(id);
  } else {
    S.selectedAgentIds.add(id);
  }
  updateSelectionPanel();
  updateSelectionHighlight();
}

export function selectAllVisibleAgents() {
  const ids = visibleAgentIds();
  setSelectedAgents(ids);
  pushLog(`SELECTED ${ids.length} visible agents`);
}

// ---------------------------------------------------------------- box select

function cancelBoxSelect() {
  S.selecting = false;
  S.selectStart = null;
  if (el.selectBox) el.selectBox.classList.remove("active");
}

function startBoxSelect(e) {
  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const { sx, sy } = viewportAtScreen(e.clientX, e.clientY);
  S.selecting = true;
  S.selectStart = {
    sx,
    sy,
    wx,
    wy,
    clientX: e.clientX,
    clientY: e.clientY,
    ctrlKey: e.ctrlKey || e.metaKey,
    add: e.shiftKey || e.ctrlKey || e.metaKey,
  };
  if (el.selectBox) {
    el.selectBox.style.left = sx + "px";
    el.selectBox.style.top = sy + "px";
    el.selectBox.style.width = "0px";
    el.selectBox.style.height = "0px";
    el.selectBox.classList.add("active");
  }
}

function updateBoxSelect(e) {
  if (!S.selecting || !S.selectStart || !el.selectBox) return;
  const { sx, sy } = viewportAtScreen(e.clientX, e.clientY);
  const left = Math.min(S.selectStart.sx, sx);
  const top = Math.min(S.selectStart.sy, sy);
  const width = Math.abs(S.selectStart.sx - sx);
  const height = Math.abs(S.selectStart.sy - sy);
  el.selectBox.style.left = left + "px";
  el.selectBox.style.top = top + "px";
  el.selectBox.style.width = width + "px";
  el.selectBox.style.height = height + "px";
}

function finishBoxSelect(e) {
  if (!S.selecting || !S.selectStart) return;
  const start = S.selectStart;
  S.selecting = false;
  S.selectStart = null;
  if (el.selectBox) el.selectBox.classList.remove("active");

  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  const pixelDx = Math.abs(e.clientX - start.clientX);
  const pixelDy = Math.abs(e.clientY - start.clientY);
  const isClick = pixelDx < 6 && pixelDy < 6;

  if (isClick) {
    // Classic RTS click: select unit under cursor, or clear if empty.
    const hit = (S.lastSnap ? S.lastSnap.entities : []).find(
      (en) => en.kind === "agent" && en.x === wx && en.y === wy,
    );
    if (hit) {
      if (start.ctrlKey) {
        toggleSelectAgent(hit.id);
      } else if (start.add && !start.ctrlKey) {
        addSelectedAgents([hit.id]);
      } else {
        setSelectedAgents([hit.id]);
      }
    } else if (!start.add && !start.ctrlKey) {
      setSelectedAgents([]);
    }
    pushLog(`SELECTED ${S.selectedAgentIds.size} agents`);
    return;
  }

  const ids = agentsInWorldRect(start.wx, start.wy, wx, wy);
  if (start.add || start.ctrlKey) {
    addSelectedAgents(ids);
  } else {
    setSelectedAgents(ids);
  }
  pushLog(`SELECTED ${S.selectedAgentIds.size} agents`);
}

// ---------------------------------------------------------------- keyboard

function dirFromKey(e) {
  switch (e.key) {
    case "ArrowLeft":
    case "a":
    case "A":
    case "h":
    case "H":
    case "4":
      return [-1, 0];
    case "ArrowRight":
    case "d":
    case "D":
    case "l":
    case "L":
    case "6":
      return [1, 0];
    case "ArrowUp":
    case "w":
    case "W":
    case "k":
    case "K":
    case "8":
      return [0, -1];
    case "ArrowDown":
    case "s":
    case "S":
    case "j":
    case "J":
    case "2":
      return [0, 1];
    default:
      return null;
  }
}

// ---------------------------------------------------------------- inspect click

function handleInspectClick(e, button) {
  if (!S.lastSnap || S.mapW <= 0 || S.mapH <= 0) return;
  const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
  if (wx < 0 || wy < 0 || wx >= S.mapW || wy >= S.mapH) return;

  if (button === 2) {
    fetchCellInspect(wx, wy);
    return;
  }

  const ents = (S.lastSnap.entities || []).filter((en) => en.x === wx && en.y === wy);
  if (ents.length) {
    // prefer agents, then monsters, then items, then resources/buildings
    const order = ["agent", "monster", "item", "tree", "iron", "hut"];
    const sorted = ents.slice().sort((a, b) => {
      const ia = order.indexOf(a.kind);
      const ib = order.indexOf(b.kind);
      if (ia !== -1 && ib !== -1) return ia - ib;
      if (ia !== -1) return -1;
      if (ib !== -1) return 1;
      return 0;
    });
    fetchEntityInspect(sorted[0].id);
  } else {
    fetchCellInspect(wx, wy);
  }
}

// ---------------------------------------------------------------- wiring

export function installInputHandlers() {
  let panning = false;
  let panLast = null;
  let panAccumX = 0;
  let panAccumY = 0;
  let panPixelDist = 0;
  let mouseDownAt = null;
  let lastClickAt = null; // for double-click mass-select

  // Mouse model (classic RTS):
  //   LMB drag  → box-select agents (no modifier)
  //   LMB click → select agent under cursor / clear if empty
  //   Shift/Ctrl+LMB → add to selection (Ctrl toggles on click)
  //   MMB / RMB drag → pan camera
  //   RMB short click → inspect cell/entity
  //   Double-click agent → select all currently visible agents
  el.viewport.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const rect = el.viewport.getBoundingClientRect();
      zoomBy(e.deltaY < 0 ? 1 : -1, e.clientX - rect.left, e.clientY - rect.top);
    },
    { passive: false },
  );

  el.viewport.addEventListener("mousedown", (e) => {
    if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;
    e.preventDefault();

    mouseDownAt = {
      x: e.clientX,
      y: e.clientY,
      t: performance.now(),
      button: e.button,
      shiftKey: e.shiftKey,
      ctrlKey: e.ctrlKey || e.metaKey,
    };

    // Middle / right button: camera pan
    if (e.button === 1 || e.button === 2) {
      panning = true;
      panLast = { x: e.clientX, y: e.clientY };
      panAccumX = 0;
      panAccumY = 0;
      panPixelDist = 0;
      el.viewport.classList.add("dragging");
      S.cam.follow = false;
      return;
    }

    // Left button: classic RTS box select (no Shift required)
    if (e.button === 0) {
      S.cam.follow = false;
      startBoxSelect(e);
    }
  });

  window.addEventListener("mousemove", (e) => {
    if (S.selecting) {
      updateBoxSelect(e);
      return;
    }
    if (!panning || !panLast) return;
    const dx = e.clientX - panLast.x;
    const dy = e.clientY - panLast.y;
    panLast = { x: e.clientX, y: e.clientY };
    panPixelDist += Math.abs(dx) + Math.abs(dy);
    panAccumX += dx;
    panAccumY += dy;
    const cs = cellSize();
    while (panAccumX >= cs) {
      S.cam.tx -= 1;
      panAccumX -= cs;
    }
    while (panAccumX <= -cs) {
      S.cam.tx += 1;
      panAccumX += cs;
    }
    while (panAccumY >= cs) {
      S.cam.ty -= 1;
      panAccumY -= cs;
    }
    while (panAccumY <= -cs) {
      S.cam.ty += 1;
      panAccumY += cs;
    }
    clampCamera();
    if (S.lastSnap) drawSnap(S.lastSnap);
  });

  window.addEventListener("mouseup", (e) => {
    if (S.selecting && e.button === 0) {
      // Double-click agent → mass-select all currently visible agents
      const { wx, wy } = worldAtScreen(e.clientX, e.clientY);
      const hit = (S.lastSnap ? S.lastSnap.entities : []).find(
        (en) => en.kind === "agent" && en.x === wx && en.y === wy,
      );
      const now = performance.now();
      const isDouble =
        hit &&
        lastClickAt &&
        now - lastClickAt.t < 320 &&
        Math.abs(e.clientX - lastClickAt.x) < 8 &&
        Math.abs(e.clientY - lastClickAt.y) < 8 &&
        Math.abs(e.clientX - (mouseDownAt ? mouseDownAt.x : e.clientX)) < 6 &&
        Math.abs(e.clientY - (mouseDownAt ? mouseDownAt.y : e.clientY)) < 6;

      if (isDouble) {
        cancelBoxSelect();
        selectAllVisibleAgents();
        lastClickAt = null;
        mouseDownAt = null;
        return;
      }

      finishBoxSelect(e);
      if (hit) {
        lastClickAt = { x: e.clientX, y: e.clientY, t: now };
      } else {
        lastClickAt = null;
      }
      mouseDownAt = null;
      return;
    }

    if (panning) {
      panning = false;
      panLast = null;
      el.viewport.classList.remove("dragging");
      const down = mouseDownAt;
      mouseDownAt = null;
      // Short right-click without pan movement → inspect
      if (down && down.button === 2 && panPixelDist < 6) {
        handleInspectClick(e, 2);
      }
      return;
    }

    mouseDownAt = null;
  });

  window.addEventListener("blur", () => {
    cancelBoxSelect();
    panning = false;
    panLast = null;
    el.viewport.classList.remove("dragging");
    mouseDownAt = null;
  });

  el.viewport.addEventListener("contextmenu", (e) => e.preventDefault());

  window.addEventListener("keydown", (e) => {
    // ignore when typing in inputs
    if (
      e.target &&
      (e.target.tagName === "INPUT" ||
        e.target.tagName === "SELECT" ||
        e.target.tagName === "TEXTAREA")
    ) {
      return;
    }

    if (e.key === "Escape") {
      e.preventDefault();
      if (el.inspectPopup && el.inspectPopup.classList.contains("visible")) {
        hideInspectPopup();
        return;
      }
      if (S.selecting) {
        cancelBoxSelect();
        return;
      }
      if (S.selectedAgentIds.size) {
        setSelectedAgents([]);
        pushLog("CLEARED selection");
      }
      return;
    }
    // Ctrl/Cmd+A → select all agents currently in FOV
    if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
      e.preventDefault();
      selectAllVisibleAgents();
      return;
    }
    if (e.code === "Space") {
      e.preventDefault();
      S.cam.follow = true;
      if (S.lastSnap) {
        const agent = focusAgent();
        if (agent) {
          centerOnTile(agent.x, agent.y);
          drawSnap(S.lastSnap);
        }
      }
      return;
    }
    if (e.key === "+" || e.key === "=") {
      zoomBy(1, el.viewport.clientWidth / 2, el.viewport.clientHeight / 2);
      return;
    }
    if (e.key === "-" || e.key === "_") {
      zoomBy(-1, el.viewport.clientWidth / 2, el.viewport.clientHeight / 2);
      return;
    }
    if (e.key === "r" || e.key === "R") {
      S.cam.zi = 4;
      S.cam.follow = true;
      syncViewSize();
      if (S.lastSnap) {
        const agent = focusAgent();
        if (agent) centerOnTile(agent.x, agent.y);
        drawSnap(S.lastSnap);
      }
      return;
    }

    // resume mock auto-play
    if (e.key === "m" || e.key === "M") {
      e.preventDefault();
      setHumanControl(false);
      pushLog("MOCK 自动");
      return;
    }

    // --- player sandbox: only move / interact / drop / rest / idle ---
    if (S.pendingDirCmd) {
      const d = dirFromKey(e);
      if (d) {
        e.preventDefault();
        sendAction({
          type: "interact",
          dx: d[0],
          dy: d[1],
          verb: S.pendingDirCmd,
        });
        pushLog(`interact ${S.pendingDirCmd} ${d[0]},${d[1]}`);
        S.pendingDirCmd = null;
      } else if (e.key === "Escape") {
        S.pendingDirCmd = null;
        pushLog("取消");
      }
      return;
    }

    // verb + direction (options also listed in snapshot.interactions)
    if (e.key === "o" || e.key === "O") {
      e.preventDefault();
      S.pendingDirCmd = "open";
      pushLog("open + 方向");
      return;
    }
    if (e.key === "c" || e.key === "C") {
      e.preventDefault();
      S.pendingDirCmd = "close";
      pushLog("close + 方向");
      return;
    }
    if (e.key === "f" || e.key === "F") {
      e.preventDefault();
      S.pendingDirCmd = "attack";
      pushLog("attack + 方向");
      return;
    }
    if (e.key === "t" || e.key === "T") {
      e.preventDefault();
      S.pendingDirCmd = "dig";
      pushLog("dig + 方向");
      return;
    }
    if (e.key === "v" || e.key === "V") {
      e.preventDefault();
      S.pendingDirCmd = "place";
      pushLog("place + 方向");
      return;
    }
    if (e.key === "u" || e.key === "U") {
      e.preventDefault();
      S.pendingDirCmd = "scoop";
      pushLog("scoop + 方向 (0,0 用 g)");
      return;
    }
    if (e.key === "n" || e.key === "N") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "plant" });
      pushLog("plant");
      return;
    }
    if (e.key === "x" || e.key === "X") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "deconstruct" });
      pushLog("deconstruct");
      return;
    }
    if (e.key === "y" || e.key === "Y") {
      e.preventDefault();
      // craft first available recipe from focused agent's interactions
      const interactions =
        (S.lastMe && S.lastMe.can && S.lastMe.can.interactions) ||
        (S.lastSnap && S.lastSnap.interactions) ||
        [];
      const craft = interactions.find((i) => i.verb === "craft");
      if (craft && craft.recipe) {
        sendAction({ type: "interact", dx: 0, dy: 0, verb: "craft", recipe: craft.recipe });
        pushLog("craft " + craft.recipe);
      } else {
        pushLog("无可用配方");
      }
      return;
    }

    // underfoot interact (default verb / single option)
    if (e.key === "g" || e.key === "G" || e.key === "Enter") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0 });
      pushLog("interact here");
      return;
    }
    if (e.key === "b" || e.key === "B") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "build" });
      pushLog("build");
      return;
    }
    if (e.key === "," || e.key === "p" || e.key === "P") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "pickup" });
      pushLog("pickup");
      return;
    }
    if (e.key === "z" || e.key === "Z") {
      e.preventDefault();
      sendAction({ type: "rest" });
      pushLog("rest");
      return;
    }
    if ((e.key === "d" || e.key === "D") && e.shiftKey) {
      e.preventDefault();
      sendAction({ type: "drop", index: 0 });
      pushLog("drop 0");
      return;
    }
    if (e.key === "." || e.key === "5") {
      e.preventDefault();
      sendAction({ type: "idle" });
      pushLog("idle");
      return;
    }
    if (e.key === ">") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "descend" });
      pushLog("descend");
      return;
    }
    if (e.key === "<") {
      e.preventDefault();
      sendAction({ type: "interact", dx: 0, dy: 0, verb: "ascend" });
      pushLog("ascend");
      return;
    }

    // movement (always player action when human keys used)
    const d = dirFromKey(e);
    if (d) {
      // In FREE cam mode, Shift+arrows pan camera instead of moving agent
      if (
        !S.cam.follow &&
        e.shiftKey &&
        (e.key.startsWith("Arrow") || ["w", "a", "s", "d", "W", "A", "S", "D"].includes(e.key))
      ) {
        e.preventDefault();
        S.cam.tx += d[0] * 2;
        S.cam.ty += d[1] * 2;
        clampCamera();
        if (S.lastSnap) drawSnap(S.lastSnap);
        return;
      }
      e.preventDefault();
      S.cam.follow = true;
      sendAction({ type: "move", dx: d[0], dy: d[1] });
    }
  });

  window.addEventListener("resize", () => {
    if (S.lastSnap) {
      if (S.cam.follow) {
        const agent = focusAgent();
        syncViewSize();
        if (agent) centerOnTile(agent.x, agent.y);
      } else {
        syncViewSize();
      }
      drawSnap(S.lastSnap);
    }
  });
}
