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
      div.className =
        "track-item" +
        (S.followToken === t.token ? " active" : "") +
        (t.invalid ? " invalid" : "");
      div.innerHTML =
        `<button type="button" class="rm" title="remove">[x]</button>` +
        `<div class="name" style="color:${t.color}">@ ${t.name || "agent"}</div>` +
        (t.invalid
          ? `<div class="meta invalid-note">✗ unknown token (server restarted?) — remove it</div>`
          : `<div class="meta">id=${t.agent_id ?? "?"}  @(${t.x ?? "?"},${t.y ?? "?"})</div>`) +
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
