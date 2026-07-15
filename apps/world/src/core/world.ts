import type {
  Action,
  ObservationMessage,
  SnapshotMessage,
  WorldEvent,
} from "@aco/protocol";
import { config } from "../config.js";
import { EntityStore } from "./entityStore.js";
import { generateMap } from "../map/generate.js";
import type { Grid } from "../map/grid.js";
import { applyAllAgentActions, type AgentActionInput } from "../actions/apply.js";
import { EventBus } from "../events/bus.js";
import { makeEvent } from "../events/types.js";
import { buildObservation } from "../observation/buildObservation.js";
import { buildSnapshot } from "../gateway/snapshot.js";

export interface WorldOptions {
  width?: number;
  height?: number;
}

/**
 * Pure-ish authoritative world: map + entities + action application.
 * No WebSocket / tick timer — those live in scheduler + gateways.
 */
export class World {
  readonly store: EntityStore;
  readonly grid: Grid;
  readonly bus: EventBus;
  tick = 0;
  focusedAgentId: string | null = null;
  private lastTickEvents: WorldEvent[] = [];

  constructor(options: WorldOptions = {}) {
    const generated = generateMap(
      options.width ?? config.mapWidth,
      options.height ?? config.mapHeight,
    );
    this.grid = generated.grid;
    this.store = new EntityStore();
    this.bus = new EventBus(200);

    this.store.add({
      id: generated.agent.id,
      type: "agent",
      components: {
        position: { x: generated.agent.x, y: generated.agent.y },
        appearance: { glyph: "A" },
        inventory: { ore: 0 },
        agentBrain: {},
      },
    });

    for (const res of generated.resources) {
      this.store.add({
        id: res.id,
        type: "resource",
        components: {
          position: { x: res.x, y: res.y },
          appearance: { glyph: "M" },
          resourceNode: { ore: res.ore },
        },
      });
    }
  }

  static create(options?: WorldOptions): World {
    return new World(options);
  }

  getAgentIds(): string[] {
    return this.store.byType("agent").map((a) => a.id);
  }

  setFocusedAgent(agentId: string | null): WorldEvent[] {
    const events: WorldEvent[] = [];
    if (this.focusedAgentId && this.focusedAgentId !== agentId) {
      const unfocus = makeEvent(
        "AGENT_UNFOCUSED",
        { agentId: this.focusedAgentId },
        this.tick,
      );
      events.push(unfocus);
      this.bus.emit(unfocus);
    }
    if (agentId && this.store.get(agentId)?.type === "agent") {
      this.focusedAgentId = agentId;
      const focus = makeEvent(
        "AGENT_FOCUSED",
        { agentId },
        this.tick,
      );
      events.push(focus);
      this.bus.emit(focus);
    } else if (agentId === null) {
      this.focusedAgentId = null;
    }
    return events;
  }

  /**
   * Apply action batches for the current tick, advance tick counter.
   * Returns all events produced this tick (including TICK_STARTED).
   */
  applyActions(
    batches: Array<{ agentId: string; actions: Action[] }>,
  ): WorldEvent[] {
    this.tick += 1;
    const events: WorldEvent[] = [];
    const started = makeEvent("TICK_STARTED", { tick: this.tick }, this.tick);
    events.push(started);

    const agentIds = new Set(this.getAgentIds());
    const inputs: AgentActionInput[] = [];
    const seen = new Set<string>();

    for (const batch of batches) {
      if (!agentIds.has(batch.agentId)) {
        events.push(
          makeEvent(
            "ACTION_REJECTED",
            {
              entityId: batch.agentId,
              action: null,
              reason: "unknown_agent",
            },
            this.tick,
          ),
        );
        continue;
      }
      if (seen.has(batch.agentId)) continue;
      seen.add(batch.agentId);
      inputs.push(batch);
    }

    // Agents without a batch idle silently (optional IDLE event skipped unless idle action sent)
    events.push(
      ...applyAllAgentActions(this.store, this.grid, inputs, this.tick),
    );

    this.lastTickEvents = events;
    this.bus.emitMany(events);
    return events;
  }

  getLastTickEvents(): WorldEvent[] {
    return this.lastTickEvents;
  }

  getSnapshot(): SnapshotMessage {
    return buildSnapshot(
      this.store,
      this.grid,
      this.tick,
      this.focusedAgentId,
      this.bus.recent(config.recentEventLimit),
    );
  }

  buildObservation(
    agentId: string,
    sinceEvents: WorldEvent[] = this.lastTickEvents,
  ): ObservationMessage {
    return buildObservation(
      this.store,
      this.grid,
      agentId,
      this.tick,
      this.focusedAgentId,
      sinceEvents,
    );
  }
}

export function createWorld(options?: WorldOptions): World {
  return World.create(options);
}