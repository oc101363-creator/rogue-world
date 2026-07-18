// tests/frontend/bus.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { on, emit, log } from "../../crates/ask-kernel/static/bus.js";

test("on/emit delivers payload to subscribers", () => {
  const seen = [];
  on("snapshot", (s) => seen.push(s.tick));
  emit("snapshot", { tick: 42 });
  assert.deepEqual(seen, [42]);
});

test("unsubscribe stops delivery", () => {
  const seen = [];
  const off = on("log", (m) => seen.push(m));
  off();
  emit("log", "nope");
  assert.deepEqual(seen, []);
});

test("log() emits the log event", () => {
  const seen = [];
  on("log", (m) => seen.push(m));
  log("hello");
  assert.deepEqual(seen, ["hello"]);
});

test("a throwing listener does not break other listeners", () => {
  const seen = [];
  on("x", () => { throw new Error("boom"); });
  on("x", () => seen.push(1));
  emit("x");
  assert.deepEqual(seen, [1]);
});
