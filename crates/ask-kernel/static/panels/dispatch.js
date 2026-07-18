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
