import type { WorldEvent } from "@aco/protocol";
import type { EntityStore } from "../core/entityStore.js";
import { makeEvent } from "../events/types.js";

/**
 * Harvest 1 ore from a resource node on the same cell as the agent.
 */
export function applyHarvest(
  store: EntityStore,
  entityId: string,
  tick: number,
): WorldEvent[] {
  const agent = store.get(entityId);
  if (!agent || agent.type !== "agent") {
    return [
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId,
          action: "harvest",
          reason: "unknown_agent",
        },
        tick,
      ),
    ];
  }

  const pos = agent.components.position;
  if (!pos) {
    return [
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId,
          action: "harvest",
          reason: "no_position",
        },
        tick,
      ),
    ];
  }

  if (!agent.components.inventory) {
    agent.components.inventory = { ore: 0 };
  }

  const resource = store
    .byType("resource")
    .find(
      (r) =>
        r.components.position?.x === pos.x &&
        r.components.position?.y === pos.y &&
        (r.components.resourceNode?.ore ?? 0) > 0,
    );

  if (!resource || !resource.components.resourceNode) {
    return [
      makeEvent(
        "ACTION_REJECTED",
        {
          entityId,
          action: "harvest",
          reason: "no_resource",
        },
        tick,
      ),
    ];
  }

  const node = resource.components.resourceNode;
  node.ore -= 1;
  agent.components.inventory.ore += 1;

  const events: WorldEvent[] = [
    makeEvent(
      "HARVESTED",
      {
        entityId,
        resourceId: resource.id,
        amount: 1,
        inventoryOre: agent.components.inventory.ore,
      },
      tick,
    ),
  ];

  if (node.ore <= 0) {
    events.push(
      makeEvent(
        "RESOURCE_DEPLETED",
        {
          resourceId: resource.id,
        },
        tick,
      ),
    );
  }

  return events;
}
