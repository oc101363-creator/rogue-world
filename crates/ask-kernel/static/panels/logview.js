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
