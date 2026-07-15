import type { WebSocket } from "ws";
import type { SnapshotMessage } from "@aco/protocol";
import type { World } from "../core/world.js";
import { PROTOCOL_VERSION } from "@aco/protocol";

export type FrontendSend = (msg: unknown) => void;

/**
 * Thin WS adapter for frontend clients.
 * In: select_agent | ping
 * Out: snapshot | hello | error | pong
 */
export class FrontendGateway {
  private clients = new Set<WebSocket>();

  constructor(private world: World) {}

  attach(ws: WebSocket): void {
    this.clients.add(ws);
    this.send(ws, {
      type: "hello",
      protocolVersion: PROTOCOL_VERSION,
      role: "frontend",
    });
    this.send(ws, this.world.getSnapshot());

    ws.on("message", (raw) => {
      try {
        const text = typeof raw === "string" ? raw : raw.toString("utf8");
        const msg = JSON.parse(text) as { type?: string; agentId?: string };
        this.handleMessage(ws, msg);
      } catch {
        this.send(ws, {
          type: "error",
          message: "invalid_json",
        });
      }
    });

    ws.on("close", () => {
      this.clients.delete(ws);
    });

    ws.on("error", () => {
      this.clients.delete(ws);
    });
  }

  private handleMessage(
    ws: WebSocket,
    msg: { type?: string; agentId?: string },
  ): void {
    switch (msg.type) {
      case "ping":
        this.send(ws, { type: "pong" });
        break;
      case "select_agent": {
        const agentId = msg.agentId;
        if (typeof agentId !== "string" || !agentId) {
          this.send(ws, {
            type: "error",
            message: "select_agent requires agentId",
          });
          return;
        }
        if (!this.world.store.has(agentId)) {
          this.send(ws, {
            type: "error",
            message: `unknown agent: ${agentId}`,
          });
          return;
        }
        this.world.setFocusedAgent(agentId);
        // Push updated snapshot to all frontends immediately
        this.broadcastSnapshot(this.world.getSnapshot());
        break;
      }
      default:
        this.send(ws, {
          type: "error",
          message: `unknown message type: ${String(msg.type)}`,
        });
    }
  }

  broadcastSnapshot(snapshot: SnapshotMessage): void {
    for (const ws of this.clients) {
      this.send(ws, snapshot);
    }
  }

  private send(ws: WebSocket, msg: unknown): void {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg));
    }
  }

  clientCount(): number {
    return this.clients.size;
  }
}
