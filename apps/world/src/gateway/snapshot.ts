import type { SnapshotMessage, WorldEvent } from "@aco/protocol";
import type { EntityStore } from "../core/entityStore.js";
import type { Grid } from "../map/grid.js";
import { tilesAsStrings } from "../map/grid.js";
import { config } from "../config.js";

export function buildSnapshot(
  store: EntityStore,
  grid: Grid,
  tick: number,
  focusedAgentId: string | null,
  recentEvents: WorldEvent[],
): SnapshotMessage {
  const entities = store.all().map((e) => {
    const p = e.components.position ?? { x: 0, y: 0 };
    const glyph = e.components.appearance?.glyph ?? "?";
    const base: SnapshotMessage["entities"][number] = {
      id: e.id,
      type: e.type,
      x: p.x,
      y: p.y,
      glyph,
    };
    if (e.type === "resource") {
      base.ore = e.components.resourceNode?.ore ?? 0;
    }
    if (e.type === "agent" && e.components.inventory) {
      base.inventory = { ore: e.components.inventory.ore };
    }
    return base;
  });

  return {
    type: "snapshot",
    tick,
    width: grid.width,
    height: grid.height,
    tiles: tilesAsStrings(grid),
    entities,
    focusedAgentId,
    recentEvents: recentEvents.slice(-config.recentEventLimit),
  };
}
