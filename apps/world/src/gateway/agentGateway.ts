import type { WebSocket } from "ws";
import {
  PROTOCOL_VERSION,
  type Action,
  type ObservationMessage,
} from "@aco/protocol";
import type { World } from "../core/world.js";
import type { TickScheduler } from "../scheduler/tick.js";

interface AgentSession {
  ws: WebSocket;
  agentId: string;
  ready: boolean;
  runtime?: "mock" | "llm";
}

/**
 * Thin WS adapter for agent runtimes.
 * Path: /ws/agent?agentId=agent-1
 * Out: hello | observation | error | pong
 * In: hello_ack | action_batch | ping
 */
export class AgentGateway {
  private sessions = new Map<string, AgentSession>();

  constructor(
    private world: World,
    private scheduler: TickScheduler,
  ) {}

  attach(ws: WebSocket, agentId: string): void {
    if (!this.world.store.has(agentId)) {
      this.send(ws, {
        type: "error",
        message: `unknown agentId: ${agentId}`,
      });
      ws.close(1008, "unknown agentId");
      return;
    }

    // Replace existing session for same agent
    const existing = this.sessions.get(agentId);
    if (existing && existing.ws !== ws) {
      try {
        existing.ws.close(1000, "replaced");
      } catch {
        /* ignore */
      }
    }

    const session: AgentSession = {
      ws,
      agentId,
      ready: false,
    };
    this.sessions.set(agentId, session);

    const map = {
      width: this.world.grid.width,
      height: this.world.grid.height,
    };

    this.send(ws, {
      type: "hello",
      protocolVersion: PROTOCOL_VERSION,
      agentId,
      map,
    });

    // Immediate observation so runtime can act before first tick if desired
    this.sendObservation(agentId);

    ws.on("message", (raw) => {
      try {
        const text = typeof raw === "string" ? raw : raw.toString("utf8");
        const msg = JSON.parse(text) as Record<string, unknown>;
        this.handleMessage(session, msg);
      } catch {
        this.send(ws, { type: "error", message: "invalid_json" });
      }
    });

    ws.on("close", () => {
      const cur = this.sessions.get(agentId);
      if (cur?.ws === ws) this.sessions.delete(agentId);
    });

    ws.on("error", () => {
      const cur = this.sessions.get(agentId);
      if (cur?.ws === ws) this.sessions.delete(agentId);
    });
  }

  private handleMessage(
    session: AgentSession,
    msg: Record<string, unknown>,
  ): void {
    const type = msg.type;
    switch (type) {
      case "ping":
        this.send(session.ws, { type: "pong" });
        break;
      case "hello_ack": {
        session.ready = true;
        if (msg.runtime === "mock" || msg.runtime === "llm") {
          session.runtime = msg.runtime;
        }
        break;
      }
      case "action_batch": {
        this.handleActionBatch(session, msg);
        break;
      }
      default:
        this.send(session.ws, {
          type: "error",
          message: `unknown message type: ${String(type)}`,
        });
    }
  }

  private handleActionBatch(
    session: AgentSession,
    msg: Record<string, unknown>,
  ): void {
    const agentId =
      typeof msg.agentId === "string" ? msg.agentId : session.agentId;
    if (agentId !== session.agentId) {
      this.send(session.ws, {
        type: "error",
        message: "agentId mismatch",
      });
      return;
    }

    const tick = msg.tick;
    if (typeof tick !== "number" || !Number.isInteger(tick)) {
      this.send(session.ws, {
        type: "error",
        message: "action_batch.tick must be integer",
      });
      return;
    }

    const actions = Array.isArray(msg.actions)
      ? (msg.actions as Action[])
      : null;
    if (!actions) {
      this.send(session.ws, {
        type: "error",
        message: "action_batch.actions must be array",
      });
      return;
    }

    const result = this.scheduler.submitActionBatch({
      agentId,
      tick,
      actions,
    });

    if (!result.ok) {
      // Do not crash; report and optionally emit reject on next tick path
      this.send(session.ws, {
        type: "error",
        message: result.reason ?? "action_batch rejected",
      });
    }
  }

  sendObservation(agentId: string, observation?: ObservationMessage): void {
    const session = this.sessions.get(agentId);
    if (!session) return;
    // Observation tick is the decision tick agents must echo in action_batch.
    const obs =
      observation ?? this.world.buildObservation(agentId);
    obs.tick = this.scheduler.decisionTick;
    this.send(session.ws, obs);
  }

  /** After each tick: push observations to all connected agents. */
  broadcastObservations(): void {
    for (const agentId of this.sessions.keys()) {
      this.sendObservation(agentId);
    }
  }

  connectedAgentIds(): string[] {
    return [...this.sessions.keys()];
  }

  private send(ws: WebSocket, msg: unknown): void {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg));
    }
  }
}
