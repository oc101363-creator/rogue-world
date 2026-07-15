import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createWorld } from "../src/core/world.js";

describe("harvest system", () => {
  it("harvests 1 ore when agent stands on a resource cell", () => {
    const world = createWorld();
    const agentId = "agent-1";
    // ore-1 is at (8, 2) with 10 ore
    world.store.setPosition(agentId, { x: 8, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [{ type: "harvest", payload: {} }],
      },
    ]);

    const agent = world.store.get(agentId)!;
    const resource = world.store.get("ore-1")!;
    assert.equal(agent.components.inventory?.ore, 1);
    assert.equal(resource.components.resourceNode?.ore, 9);
    assert.ok(events.some((e) => e.type === "HARVESTED"));
  });

  it("rejects harvest when not on a resource", () => {
    const world = createWorld();
    const agentId = "agent-1";
    world.store.setPosition(agentId, { x: 2, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [{ type: "harvest", payload: {} }],
      },
    ]);

    assert.equal(world.store.get(agentId)!.components.inventory?.ore, 0);
    assert.ok(
      events.some(
        (e) =>
          e.type === "ACTION_REJECTED" &&
          (e.payload as { reason?: string }).reason === "no_resource",
      ),
    );
  });

  it("emits RESOURCE_DEPLETED when last ore is taken", () => {
    const world = createWorld();
    const agentId = "agent-1";
    world.store.setPosition(agentId, { x: 8, y: 2 });
    const resource = world.store.get("ore-1")!;
    resource.components.resourceNode!.ore = 1;

    const events = world.applyActions([
      {
        agentId,
        actions: [{ type: "harvest", payload: {} }],
      },
    ]);

    assert.equal(resource.components.resourceNode?.ore, 0);
    assert.equal(world.store.get(agentId)!.components.inventory?.ore, 1);
    assert.ok(events.some((e) => e.type === "HARVESTED"));
    assert.ok(events.some((e) => e.type === "RESOURCE_DEPLETED"));
  });

  it("say may accompany harvest without being a second mutator", () => {
    const world = createWorld();
    const agentId = "agent-1";
    world.store.setPosition(agentId, { x: 8, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [
          { type: "harvest", payload: {} },
          { type: "say", payload: { text: "mining" } },
        ],
      },
    ]);

    assert.equal(world.store.get(agentId)!.components.inventory?.ore, 1);
    assert.ok(events.some((e) => e.type === "HARVESTED"));
    assert.ok(events.some((e) => e.type === "AGENT_SAID"));
  });
});
