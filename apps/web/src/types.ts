/** Minimal frontend types mirroring packages/protocol SnapshotMessage. */

export interface WorldEvent {
  type: string;
  payload: Record<string, unknown>;
  tick?: number;
}

export interface SnapshotEntity {
  id: string;
  type: "agent" | "resource";
  x: number;
  y: number;
  glyph: string;
  ore?: number;
  inventory?: { ore: number };
}

export interface SnapshotMessage {
  type: "snapshot";
  tick: number;
  width: number;
  height: number;
  tiles: string[];
  entities: SnapshotEntity[];
  focusedAgentId: string | null;
  recentEvents: WorldEvent[];
}

export interface HelloMessage {
  type: "hello";
  protocolVersion?: string;
  [key: string]: unknown;
}

export interface ErrorMessage {
  type: "error";
  message?: string;
  [key: string]: unknown;
}

export interface PongMessage {
  type: "pong";
  [key: string]: unknown;
}

export type ServerMessage =
  | SnapshotMessage
  | HelloMessage
  | ErrorMessage
  | PongMessage
  | { type: string; [key: string]: unknown };

export type ConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  | "error";

export interface SelectAgentMessage {
  type: "select_agent";
  agentId: string;
}

export interface PingMessage {
  type: "ping";
}
