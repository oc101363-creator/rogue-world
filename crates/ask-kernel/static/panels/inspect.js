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
