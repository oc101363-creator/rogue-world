import type { WorldEvent } from "@aco/protocol";

type Listener = (event: WorldEvent) => void;

/** Simple in-process event bus for live listeners (gateways, debug). */
export class EventBus {
  private listeners = new Set<Listener>();
  private history: WorldEvent[] = [];
  private maxHistory: number;

  constructor(maxHistory = 200) {
    this.maxHistory = maxHistory;
  }

  on(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit(event: WorldEvent): void {
    this.history.push(event);
    if (this.history.length > this.maxHistory) {
      this.history.splice(0, this.history.length - this.maxHistory);
    }
    for (const listener of this.listeners) {
      listener(event);
    }
  }

  emitMany(events: WorldEvent[]): void {
    for (const e of events) this.emit(e);
  }

  recent(limit = 20): WorldEvent[] {
    return this.history.slice(-limit);
  }

  clearHistory(): void {
    this.history = [];
  }
}
