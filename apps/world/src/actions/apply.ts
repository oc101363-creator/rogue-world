import type { Action, WorldEvent } from "@aco/protocol";
import type { EntityStore } from "../core/entityStore.js";
import type { Grid } from "../map/grid.js";
import { applyMove } from "../systems/movement.js";
import { applyHarvest } from "../systems/harvest.js";
import { makeEvent } from "../events/types.js";
import { validateActionBatch } from "./validate.js";

export interface AgentActionInput {
  agentId: string;
  actions: Action[];
}

/**
 * Validate and apply one agent's action batch for a tick.
 * At most one mutator; says may accompany.
 */
export function applyAgentActions(
  store: EntityStore,
  grid: Grid,
  agentId: string,
  actions: Action[],
  tick: number,
): WorldEvent[] {
  const events: WorldEvent[] = [];
  const validated = validateActionBatch(actions);

  for (const rej of validated.rejected) {
    events.push(
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId: agentId,
          action: rej.action,
          reason: rej.reason,
        },
        tick,
      ),
    );
  }

  if (validated.mutator) {
    events.push(...applyMutator(store, grid, agentId, validated.mutator, tick));
  }

  for (const say of validated.says) {
    if (say.type !== "say") continue;
    events.push(
      makeEvent(
        "AGENT_SAID",
        {
          entityId: agentId,
          text: say.payload.text,
        },
        tick,
      ),
    );
  }

  return events;
}

function applyMutator(
  store: EntityStore,
  grid: Grid,
  agentId: string,
  action: Action,
  tick: number,
): WorldEvent[] {
  switch (action.type) {
    case "move":
      return applyMove(
        store,
        grid,
        {
          entityId: agentId,
          dx: action.payload.dx,
          dy: action.payload.dy,
        },
        tick,
      );
    case "harvest":
      return applyHarvest(store, agentId, tick);
    case "idle":
      return [
        makeEvent(
          "IDLE",
          {
            entityId: agentId,
          },
          tick,
        ),
      ];
    default:
      return [
        makeEvent(
          "ACTION_REJECTED",
          {
            entityId: agentId,
            action,
            reason: "not_a_mutator",
          },
          tick,
        ),
      ];
  }
}

/**
 * Apply multiple agents' batches deterministically (sorted by agentId).
 */
export function applyAllAgentActions(
  store: EntityStore,
  grid: Grid,
  batches: AgentActionInput[],
  tick: number,
): WorldEvent[] {
  const sorted = [...batches].sort((a, b) =>
    a.agentId.localeCompare(b.agentId),
  );
  const events: WorldEvent[] = [];
  for (const batch of sorted) {
    events.push(
      ...applyAgentActions(store, grid, batch.agentId, batch.actions, tick),
    );
  }
  return events;
}
