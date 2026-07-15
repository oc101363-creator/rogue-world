import type { WorldEvent } from "@aco/protocol";
import type { EntityStore } from "../core/entityStore.js";
import type { Grid } from "../map/grid.js";
import { isWalkable } from "../map/grid.js";
import { makeEvent } from "../events/types.js";

export interface MoveIntent {
  entityId: string;
  dx: number;
  dy: number;
}

/**
 * Four-way step: |dx|+|dy| must be 1; destination must be walkable floor.
 * Does not block on other entities in V1 (only walls/OOB).
 */
export function applyMove(
  store: EntityStore,
  grid: Grid,
  intent: MoveIntent,
  tick: number,
): WorldEvent[] {
  const entity = store.get(intent.entityId);
  if (!entity || entity.type !== "agent") {
    return [
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId: intent.entityId,
          action: "move",
          reason: "unknown_agent",
        },
        tick,
      ),
    ];
  }

  const pos = entity.components.position;
  if (!pos) {
    return [
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId: intent.entityId,
          action: "move",
          reason: "no_position",
        },
        tick,
      ),
    ];
  }

  const { dx, dy } = intent;
  if (!Number.isInteger(dx) || !Number.isInteger(dy) || Math.abs(dx) + Math.abs(dy) !== 1) {
    return [
      makeEvent(
        "MOVE_FAILED",
        {
          entityId: intent.entityId,
          reason: "invalid_delta",
        },
        tick,
      ),
    ];
  }

  const to = { x: pos.x + dx, y: pos.y + dy };
  if (!isWalkable(grid, to.x, to.y)) {
    return [
      makeEvent(
        "MOVE_FAILED",
        {
          entityId: intent.entityId,
          reason: "blocked",
          from: { ...pos },
          attempted: to,
        },
        tick,
      ),
    ];
  }

  const from = { x: pos.x, y: pos.y };
  store.setPosition(intent.entityId, to);
  return [
    makeEvent(
      "MOVED",
      {
        entityId: intent.entityId,
        from,
        to,
      },
      tick,
    ),
  ];
}
