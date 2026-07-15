import { useEffect, useRef } from "react";
import * as ROT from "rot-js";
import type { SnapshotMessage } from "../types";

export interface RotMapProps {
  snapshot: SnapshotMessage | null;
  onSelect: (agentId: string) => void;
}

const TILE_FG: Record<string, string> = {
  "#": "#5a7a5a",
  ".": "#1a3a1a",
};

const TILE_BG: Record<string, string> = {
  "#": "#0a120a",
  ".": "#050805",
};

/**
 * rot.js Display map: draws terrain tiles then entities.
 * Focused agent uses glyph @ with amber highlight.
 * Click a cell with an agent → onSelect(agentId).
 */
export function RotMap({ snapshot, onSelect }: RotMapProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const displayRef = useRef<ROT.Display | null>(null);
  const snapshotRef = useRef<SnapshotMessage | null>(null);
  const onSelectRef = useRef(onSelect);

  snapshotRef.current = snapshot;
  onSelectRef.current = onSelect;

  // Create display once.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const display = new ROT.Display({
      width: 16,
      height: 12,
      fontSize: 20,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      bg: "#050805",
      fg: "#33ff66",
      forceSquareRatio: true,
    });
    displayRef.current = display;
    el.innerHTML = "";
    el.appendChild(display.getContainer()!);

    const handleClick = (ev: MouseEvent) => {
      const snap = snapshotRef.current;
      if (!snap) return;
      const pos = display.eventToPosition(ev);
      if (!pos || pos[0] < 0 || pos[1] < 0) return;
      const [x, y] = pos;
      const agent = snap.entities.find(
        (e) => e.type === "agent" && e.x === x && e.y === y,
      );
      if (agent) {
        onSelectRef.current(agent.id);
      }
    };

    const canvas = display.getContainer();
    canvas?.addEventListener("click", handleClick);

    return () => {
      canvas?.removeEventListener("click", handleClick);
      displayRef.current = null;
      el.innerHTML = "";
    };
  }, []);

  // Redraw when snapshot changes; resize display if map size changes.
  useEffect(() => {
    const display = displayRef.current;
    if (!display || !snapshot) return;

    const { width, height, tiles, entities, focusedAgentId } = snapshot;
    const opts = display.getOptions();
    if (opts.width !== width || opts.height !== height) {
      display.setOptions({ width, height });
    }

    display.clear();

    for (let y = 0; y < height; y++) {
      const row = tiles[y] ?? "";
      for (let x = 0; x < width; x++) {
        const ch = row[x] ?? " ";
        display.draw(
          x,
          y,
          ch,
          TILE_FG[ch] ?? "#2a4a2a",
          TILE_BG[ch] ?? "#050805",
        );
      }
    }

    // Draw resources first, agents on top.
    const resources = entities.filter((e) => e.type === "resource");
    const agents = entities.filter((e) => e.type === "agent");

    for (const e of resources) {
      display.draw(e.x, e.y, e.glyph || "M", "#ffcc33", TILE_BG["."] ?? "#050805");
    }

    for (const e of agents) {
      const focused = focusedAgentId !== null && e.id === focusedAgentId;
      const glyph = focused ? "@" : e.glyph || "A";
      const fg = focused ? "#ffb000" : "#33ff66";
      display.draw(e.x, e.y, glyph, fg, "#0a2010");
    }
  }, [snapshot]);

  return (
    <div className="rot-map" ref={containerRef} aria-label="World map" />
  );
}
