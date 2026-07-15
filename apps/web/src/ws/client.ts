import type {
  ConnectionStatus,
  SelectAgentMessage,
  ServerMessage,
  SnapshotMessage,
} from "../types";

const DEFAULT_WS_URL = "ws://127.0.0.1:8080/ws/frontend";

export type SnapshotHandler = (snapshot: SnapshotMessage) => void;
export type StatusHandler = (status: ConnectionStatus, detail?: string) => void;

export interface FrontendWsClient {
  connect: () => void;
  disconnect: () => void;
  selectAgent: (agentId: string) => void;
  ping: () => void;
}

/**
 * WebSocket client for World frontend gateway.
 * Reconnects with simple exponential backoff on disconnect.
 */
export function createFrontendWsClient(options: {
  url?: string;
  onSnapshot: SnapshotHandler;
  onStatus: StatusHandler;
  onMessage?: (msg: ServerMessage) => void;
}): FrontendWsClient {
  const url =
    options.url ??
    (import.meta.env.VITE_WS_URL as string | undefined) ??
    DEFAULT_WS_URL;

  let ws: WebSocket | null = null;
  let closedByUser = false;
  let attempt = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let pingTimer: ReturnType<typeof setInterval> | null = null;

  const clearTimers = () => {
    if (reconnectTimer !== null) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (pingTimer !== null) {
      clearInterval(pingTimer);
      pingTimer = null;
    }
  };

  const scheduleReconnect = () => {
    if (closedByUser) return;
    const delay = Math.min(1000 * 2 ** attempt, 15000);
    attempt += 1;
    options.onStatus("connecting", `reconnect in ${delay}ms`);
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, delay);
  };

  const startPing = () => {
    if (pingTimer !== null) clearInterval(pingTimer);
    pingTimer = setInterval(() => {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "ping" }));
      }
    }, 15000);
  };

  const connect = () => {
    clearTimers();
    closedByUser = false;
    options.onStatus("connecting");

    try {
      ws = new WebSocket(url);
    } catch (err) {
      options.onStatus(
        "error",
        err instanceof Error ? err.message : "WebSocket construct failed",
      );
      scheduleReconnect();
      return;
    }

    ws.onopen = () => {
      attempt = 0;
      options.onStatus("connected");
      startPing();
    };

    ws.onmessage = (ev) => {
      let msg: ServerMessage;
      try {
        msg = JSON.parse(String(ev.data)) as ServerMessage;
      } catch {
        options.onStatus("error", "invalid JSON from server");
        return;
      }

      options.onMessage?.(msg);

      switch (msg.type) {
        case "snapshot":
          options.onSnapshot(msg as SnapshotMessage);
          break;
        case "hello":
          // Session greeting; no UI action required beyond status.
          break;
        case "pong":
          break;
        case "error": {
          const errMsg =
            typeof (msg as { message?: unknown }).message === "string"
              ? (msg as { message: string }).message
              : "server error";
          options.onStatus("error", errMsg);
          break;
        }
        default:
          break;
      }
    };

    ws.onerror = () => {
      // onclose usually follows; surface a soft error state.
      options.onStatus("error", "websocket error");
    };

    ws.onclose = () => {
      ws = null;
      if (pingTimer !== null) {
        clearInterval(pingTimer);
        pingTimer = null;
      }
      if (!closedByUser) {
        options.onStatus("disconnected");
        scheduleReconnect();
      } else {
        options.onStatus("disconnected", "closed");
      }
    };
  };

  const disconnect = () => {
    closedByUser = true;
    clearTimers();
    if (ws) {
      ws.close();
      ws = null;
    }
    options.onStatus("disconnected", "closed");
  };

  const send = (payload: object) => {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify(payload));
  };

  const selectAgent = (agentId: string) => {
    const msg: SelectAgentMessage = { type: "select_agent", agentId };
    send(msg);
  };

  const ping = () => {
    send({ type: "ping" });
  };

  return { connect, disconnect, selectAgent, ping };
}
