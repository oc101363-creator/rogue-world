/* ASK viewer — entry point: wire buttons, install handlers, connect.
 * Modules: state.js (shared state) · render.js (drawing) · net.js (server)
 * · input.js (keyboard/mouse). This file only assembles them. */

import { el, S, loadPresets, savePresets, loadSquads, saveSquads } from "./state.js";
import {
  renderPresets,
  updateSelectionPanel,
  pushLog,
  hideInspectPopup,
} from "./render.js";
import {
  connect,
  refreshTracked,
  sendPromptToSelected,
  fetchOperatorInbox,
} from "./net.js";
import {
  installInputHandlers,
  selectAllVisibleAgents,
  setSelectedAgents,
} from "./input.js";
import { mountMapview } from "./mapview.js";
import { mountHud } from "./panels/hud.js";
import { mountTracker } from "./panels/tracker.js";

// inspect popup
if (el.inspectClose) {
  el.inspectClose.addEventListener("click", (e) => {
    e.stopPropagation();
    hideInspectPopup();
  });
}

// prompt presets
if (el.selPreset) {
  el.selPreset.addEventListener("change", () => {
    const id = el.selPreset.value;
    if (!id) return;
    const p = loadPresets().find((x) => x.id === id);
    if (p && el.selText) el.selText.value = p.text;
  });
}
if (el.selPresetSave) {
  el.selPresetSave.addEventListener("click", () => {
    const name = ((el.selPresetName && el.selPresetName.value) || "").trim();
    const text = ((el.selText && el.selText.value) || "").trim();
    if (!name || !text) {
      pushLog("PRESET: need name and text");
      return;
    }
    const presets = loadPresets();
    presets.push({ id: Date.now().toString(36), name, text });
    savePresets(presets);
    renderPresets();
    if (el.selPresetName) el.selPresetName.value = "";
    pushLog(`PRESET saved: ${name}`);
  });
}
if (el.selPresetDel) {
  el.selPresetDel.addEventListener("click", () => {
    const id = el.selPreset.value;
    if (!id) return;
    const presets = loadPresets().filter((p) => p.id !== id);
    savePresets(presets);
    renderPresets();
    if (el.selText) el.selText.value = "";
    pushLog("PRESET deleted");
  });
}
if (el.selSend) {
  el.selSend.addEventListener("click", () => {
    sendPromptToSelected(el.selText ? el.selText.value : "");
  });
}
if (el.selAllVis) {
  el.selAllVis.addEventListener("click", () => {
    selectAllVisibleAgents();
  });
}
if (el.selClear) {
  el.selClear.addEventListener("click", () => {
    setSelectedAgents([]);
    pushLog("CLEARED selection");
  });
}

// named squads (selection sets, localStorage only — the server never knows)
function renderSquads() {
  if (!el.squadList) return;
  const squads = loadSquads();
  el.squadList.innerHTML = "";
  const empty = document.createElement("option");
  empty.value = "";
  empty.textContent = squads.length ? "(squads)" : "(no squads)";
  el.squadList.appendChild(empty);
  for (const sq of squads) {
    const opt = document.createElement("option");
    opt.value = sq.name;
    opt.textContent = `${sq.name} (${sq.ids.length})`;
    el.squadList.appendChild(opt);
  }
}
if (el.squadSave) {
  el.squadSave.addEventListener("click", () => {
    const name = (el.squadName.value || "").trim();
    if (!name) return pushLog("SQUAD: name it first");
    if (!S.selectedAgentIds.size) return pushLog("SQUAD: empty selection");
    const squads = loadSquads().filter((s) => s.name !== name);
    squads.push({ name, ids: Array.from(S.selectedAgentIds) });
    saveSquads(squads);
    el.squadName.value = "";
    renderSquads();
    pushLog(`SQUAD saved "${name}" (${S.selectedAgentIds.size})`);
  });
}
if (el.squadLoad) {
  el.squadLoad.addEventListener("click", () => {
    const sq = loadSquads().find((s) => s.name === el.squadList.value);
    if (!sq) return;
    setSelectedAgents(sq.ids);
    pushLog(`SQUAD "${sq.name}" → ${sq.ids.length} selected`);
  });
}
if (el.squadDel) {
  el.squadDel.addEventListener("click", () => {
    const name = el.squadList.value;
    if (!name) return;
    saveSquads(loadSquads().filter((s) => s.name !== name));
    renderSquads();
    pushLog(`SQUAD deleted "${name}"`);
  });
}
if (el.opInbox) {
  el.opInbox.addEventListener("click", () => {
    fetchOperatorInbox();
  });
}

// boot
mountMapview(el.map);
mountHud(document.getElementById("hud"));
mountTracker(document.getElementById("dock-track"));
renderPresets();
renderSquads();
updateSelectionPanel();
refreshTracked();
installInputHandlers();
connect();
