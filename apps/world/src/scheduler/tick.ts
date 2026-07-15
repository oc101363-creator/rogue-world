import type { Action, WorldEvent } from "@aco/protocol";
import type { World } from "../core/world.js";
import type { EventLogDb } from "../persist/db.js";
import { config } from "../config.js";

export type PendingBatch = {
  agentId: string;
  tick: number;
  actions: Action[];
};

export interface TickSchedulerOptions {
  world: World;
  db?: EventLogDb | null;
  tickMs?: number;
  decisionTimeoutMs?: number;
  onTickComplete?: (info: {
    tick: number;
    events: WorldEvent[];
  }) => void;
}

/**
 * Fixed-rate tick loop.
 * Collects pending action batches for the *next* decision tick,
 * applies them on tick boundary, persists, and notifies listeners.
 */
export class TickScheduler {
  readonly world: World;
  private db: EventLogDb | null;
  private tickMs: number;
  private decisionTimeoutMs: number;
  private onTickComplete?: TickSchedulerOptions["onTickComplete"];

  /** Actions collected for the upcoming apply tick. */
  private pending = new Map<string, PendingBatch>();
  /** Decision tick agents should target in action_batch.tick. */
  decisionTick = 1;
  private timer: ReturnType<typeof setInterval> | null = null;
  private running = false;

  constructor(options: TickSchedulerOptions) {
    this.world = options.world;
    this.db = options.db ?? null;
    this.tickMs = options.tickMs ?? config.tickMs;
    this.decisionTimeoutMs =
      options.decisionTimeoutMs ?? config.decisionTimeoutMs;
    this.onTickComplete = options.onTickComplete;
  }

  setOnTickComplete(cb: TickSchedulerOptions["onTickComplete"]): void {
    this.onTickComplete = cb;
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    // Initial observation window: agents may submit for tick 1 before first fire
    this.timer = setInterval(() => this.tickOnce(), this.tickMs);
  }

  stop(): void {
    this.running = false;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  /**
   * Submit action_batch from agent gateway.
   * Only accepts batches whose tick matches current decisionTick.
   */
  submitActionBatch(batch: PendingBatch): {
    ok: boolean;
    reason?: string;
  } {
    if (batch.tick !== this.decisionTick) {
      return {
        ok: false,
        reason: `stale_or_future_tick: expected ${this.decisionTick}, got ${batch.tick}`,
      };
    }
    if (!this.world.store.has(batch.agentId)) {
      return { ok: false, reason: "unknown_agent" };
    }
    // Last write wins within the decision window
    this.pending.set(batch.agentId, {
      agentId: batch.agentId,
      tick: batch.tick,
      actions: batch.actions,
    });
    return { ok: true };
  }

  getPending(agentId: string): PendingBatch | undefined {
    return this.pending.get(agentId);
  }

  /** Run one tick immediately (also used by tests). */
  tickOnce(): WorldEvent[] {
    const batches = [...this.pending.values()].map((p) => ({
      agentId: p.agentId,
      actions: p.actions,
    }));
    this.pending.clear();

    const events = this.world.applyActions(batches);

    if (this.db) {
      try {
        this.db.insertEvents(events);
        if (
          this.world.tick > 0 &&
          this.world.tick % config.snapshotEveryNTicks === 0
        ) {
          this.db.insertSnapshot(this.world.tick, this.world.getSnapshot());
        }
      } catch (err) {
        console.error("[tick] persist failed", err);
      }
    }

    // Next decision window targets the next tick number
    this.decisionTick = this.world.tick + 1;

    this.onTickComplete?.({ tick: this.world.tick, events });
    return events;
  }
}
