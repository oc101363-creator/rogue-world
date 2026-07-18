import { test } from "node:test";
import assert from "node:assert/strict";

// state.js touches localStorage/document at module scope — stub before import
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
globalThis.location = { protocol: "http:", host: "x" };
globalThis.document = { getElementById: () => null };

const { S, setSelectedAgents, addSelectedAgents, toggleSelectAgent } =
  await import("../../crates/ask-kernel/static/state.js");
const { on } = await import("../../crates/ask-kernel/static/bus.js");

test("selection mutators update the set and emit", () => {
  let fires = 0;
  on("selection-changed", () => fires++);
  setSelectedAgents([1, 2]);
  assert.deepEqual([...S.selectedAgentIds].sort(), [1, 2]);
  addSelectedAgents([3]);
  toggleSelectAgent(1); // remove
  toggleSelectAgent(4); // add
  assert.deepEqual([...S.selectedAgentIds].sort(), [2, 3, 4]);
  assert.equal(fires, 4);
});
