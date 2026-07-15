import {
  ALLOWED_ACTIONS,
  PROTOCOL_VERSION,
  type ObservationMessage,
  type WorldEvent,
} from "@aco/protocol";
import type { EntityStore } from "../core/entityStore.js";
import type { Grid } from "../map/grid.js";
import { tilesAsStrings } from "../map/grid.js";

export function buildObservation(
  store: EntityStore,
  grid: Grid,
  agentId: string,
  tick: number,
  focusedAgentId: string | null,
  events: WorldEvent[],
  goal: string | null = null,
): ObservationMessage {
  const agent = store.get(agentId);
  const pos = agent?.components.position ?? { x: 0, y: 0 };
  const inventory = agent?.components.inventory ?? { ore: 0 };

  const entities = store.all().map((e) => {
    const p = e.components.position ?? { x: 0, y: 0 };
    const glyph = e.components.appearance?.glyph ?? "?";
    const base = {
      id: e.id,
      type: e.type,
      x: p.x,
      y: p.y,
      glyph,
    };
    if (e.type === "resource") {
      return { ...base, ore: e.components.resourceNode?.ore ?? 0 };
    }
    return base;
  });

  return {
    type: "observation",
    protocolVersion: PROTOCOL_VERSION,
    tick,
    self: {
      id: agentId,
      x: pos.x,
      y: pos.y,
      inventory: { ore: inventory.ore },
    },
    visible: {
      width: grid.width,
      height: grid.height,
      tiles: tilesAsStrings(grid),
      entities,
    },
    events,
    allowed_actions: [...ALLOWED_ACTIONS],
    focused: focusedAgentId === agentId,
    goal,
  };
}
