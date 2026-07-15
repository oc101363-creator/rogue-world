import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createWorld } from "../src/core/world.js";
import type { Action } from "@aco/protocol";

describe("movement system", () => {
  it("moves agent one step east when path is clear", () => {
    const world = createWorld();
    const agentId = "agent-1";
    const before = world.store.getPosition(agentId)!;
    // Place agent at (2,2) for a clear east move inside the map
    world.store.setPosition(agentId, { x: 2, y: 2 });

    const actions: Action[] = [
      { type: "move", payload: { dx: 1, dy: 0 } },
    ];
    const events = world.applyActions([{ agentId, actions }]);

    const after = world.store.getPosition(agentId)!;
    assert.equal(after.x, 3);
    assert.equal(after.y, 2);
    assert.ok(events.some((e) => e.type === "MOVED"));
    assert.equal(before.x, 2); // original spawn was elsewhere; we forced (2,2)
  });

  it("fails when moving into a wall", () => {
    const world = createWorld();
    const agentId = "agent-1";
    // Left border is wall at x=0; put agent at x=1 and move west
    world.store.setPosition(agentId, { x: 1, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [{ type: "move", payload: { dx: -1, dy: 0 } }],
      },
    ]);

    const after = world.store.getPosition(agentId)!;
    assert.equal(after.x, 1);
    assert.equal(after.y, 2);
    assert.ok(events.some((e) => e.type === "MOVE_FAILED"));
    assert.ok(!events.some((e) => e.type === "MOVED"));
  });

  it("rejects diagonal moves", () => {
    const world = createWorld();
    const agentId = "agent-1";
    world.store.setPosition(agentId, { x: 2, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [{ type: "move", payload: { dx: 1, dy: 1 } }],
      },
    ]);

    const after = world.store.getPosition(agentId)!;
    assert.equal(after.x, 2);
    assert.equal(after.y, 2);
    assert.ok(
      events.some(
        (e) => e.type === "ACTION_REJECTED" || e.type === "MOVE_FAILED",
      ),
    );
  });

  it("allows only one mutator per tick; extra is rejected", () => {
    const world = createWorld();
    const agentId = "agent-1";
    world.store.setPosition(agentId, { x: 2, y: 2 });

    const events = world.applyActions([
      {
        agentId,
        actions: [
          { type: "move", payload: { dx: 1, dy: 0 } },
          { type: "move", payload: { dx: 1, dy: 0 } },
        ],
      },
    ]);

    const after = world.store.getPosition(agentId)!;
    assert.equal(after.x, 3);
    assert.equal(after.y, 2);
    assert.ok(events.some((e) => e.type === "MOVED"));
    assert.ok(
      events.some(
        (e) =>
          e.type === "ACTION_REJECTED" &&
          (e.payload as { reason?: string }).reason === "extra_mutator",
      ),
    );
  });
});
