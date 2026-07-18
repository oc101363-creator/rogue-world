// scripts/check-frontend.mjs
/* Module link check: import every frontend module with browser globals
 * stubbed. Catches bad import paths, missing exports, and top-level
 * reference errors — the frontend's compile step. */
const elStub = () => ({
  style: { setProperty() {} },
  classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
  appendChild() {}, addEventListener() {}, insertBefore() {}, firstChild: null,
  removeChild() {}, querySelectorAll: () => [], querySelector: () => elStub(),
  set innerHTML(v) {}, get innerHTML() { return ""; },
  textContent: "", value: "", getContext: () => null,
});
globalThis.location = { protocol: "http:", host: "x" };
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
globalThis.document = {
  getElementById: elStub, createElement: elStub,
  documentElement: { style: { setProperty() {} } },
  body: { style: {}, classList: { add() {}, remove() {}, toggle() {}, contains: () => false } },
};
globalThis.window = { addEventListener() {} };
globalThis.requestAnimationFrame = () => {};
// No network in the link check: app.js boots connect() at import time.
globalThis.WebSocket = class {
  close() {} send() {}
};
globalThis.fetch = () => new Promise(() => {});

const base = new URL("../crates/ask-kernel/static/", import.meta.url);
const modules = process.argv.slice(2);
if (!modules.length) {
  console.error("usage: node scripts/check-frontend.mjs <module.js>…");
  process.exit(2);
}
for (const m of modules) {
  await import(new URL(m, base));
  console.log("ok", m);
}
console.log("module graph links OK");
