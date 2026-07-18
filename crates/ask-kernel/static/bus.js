// crates/ask-kernel/static/bus.js
/* Tiny pub/sub — the ONLY cross-module channel besides state.js.
 * Breaks the old render↔net import cycle: producers emit,
 * consumers subscribe, neither imports the other. */

const listeners = new Map(); // event -> Set<fn>

/** Subscribe. Returns the unsubscribe function. */
export function on(event, fn) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event).add(fn);
  return () => listeners.get(event)?.delete(fn);
}

/** Publish. A throwing listener is logged and skipped, never fatal. */
export function emit(event, payload) {
  for (const fn of listeners.get(event) ?? []) {
    try {
      fn(payload);
    } catch (e) {
      console.error(`[bus] listener for "${event}" threw:`, e);
    }
  }
}

/** Every "pushLog" call site in the old code becomes this. */
export function log(msg) {
  emit("log", msg);
}
