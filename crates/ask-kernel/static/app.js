/* ASK viewer — entry point: wire buttons, install handlers, connect.
 * Modules: state.js (shared state) · render.js (drawing) · net.js (server)
 * · input.js (keyboard/mouse). This file only assembles them. */

import { el, S, loadPresets, savePresets } from "./state.js";
import {
  setupThemeSelect,
  updateModeHud,
  renderPresets,
  updateSelectionPanel,
  renderTracker,
  pushLog,
  hideInspectPopup,
} from "./render.js";
import {
  connect,
  addToken,
  clearTracked,
  refreshTracked,
  setHumanControl,
  sendPromptToSelected,
  applySnapshot,
} from "./net.js";
import {
  installInputHandlers,
  selectAllVisibleAgents,
  setSelectedAgents,
} from "./input.js";

// token panel
if (el.tokenAdd) {
  el.tokenAdd.addEventListener("click", () => {
    addToken(el.tokenInput && el.tokenInput.value);
    if (el.tokenInput) el.tokenInput.value = "";
  });
}
if (el.tokenClear) {
  el.tokenClear.addEventListener("click", () => {
    clearTracked();
  });
}
if (el.tokenInput) {
  el.tokenInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addToken(el.tokenInput.value);
      el.tokenInput.value = "";
    }
  });
}
if (el.btnFollow) {
  el.btnFollow.addEventListener("click", () => {
    S.cam.follow = true;
    pushLog("FOLLOW ON");
    if (S.lastSnap) applySnapshot(S.lastSnap);
  });
}
if (el.btnMock) {
  el.btnMock.addEventListener("click", () => {
    setHumanControl(false);
    pushLog("MOCK");
  });
}

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

// boot
setupThemeSelect();
updateModeHud();
renderPresets();
updateSelectionPanel();
renderTracker();
refreshTracked();
installInputHandlers();
connect();
