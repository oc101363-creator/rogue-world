import http from "node:http";
import { WebSocketServer, type WebSocket } from "ws";
import { URL } from "node:url";
import type { World } from "../core/world.js";
import type { TickScheduler } from "../scheduler/tick.js";
import { FrontendGateway } from "./frontendGateway.js";
import { AgentGateway } from "./agentGateway.js";
import { createHttpHandler } from "./httpApi.js";
import { config } from "../config.js";

export interface WorldServer {
  httpServer: http.Server;
  frontend: FrontendGateway;
  agents: AgentGateway;
  close: () => Promise<void>;
}

export function createWorldServer(
  world: World,
  scheduler: TickScheduler,
  port = config.port,
): WorldServer {
  const frontend = new FrontendGateway(world);
  const agents = new AgentGateway(world, scheduler);
  const httpHandler = createHttpHandler(world, scheduler, frontend);

  const httpServer = http.createServer((req, res) => {
    httpHandler(req, res);
  });

  const wss = new WebSocketServer({ noServer: true });

  httpServer.on("upgrade", (request, socket, head) => {
    const host = request.headers.host ?? "localhost";
    const url = new URL(request.url ?? "/", `http://${host}`);
    const pathname = url.pathname;

    if (pathname === "/ws/frontend") {
      wss.handleUpgrade(request, socket, head, (ws: WebSocket) => {
        frontend.attach(ws);
      });
      return;
    }

    if (pathname === "/ws/agent") {
      const agentId = url.searchParams.get("agentId") ?? config.agentId;
      wss.handleUpgrade(request, socket, head, (ws: WebSocket) => {
        agents.attach(ws, agentId);
      });
      return;
    }

    socket.write("HTTP/1.1 404 Not Found\r\n\r\n");
    socket.destroy();
  });

  httpServer.listen(port, () => {
    console.log(`[world] listening on http://127.0.0.1:${port}`);
    console.log(`[world] HTTP API   http://127.0.0.1:${port}/api/status`);
    console.log(`[world] frontend WS  ws://127.0.0.1:${port}/ws/frontend`);
    console.log(
      `[world] agent WS     ws://127.0.0.1:${port}/ws/agent?agentId=${config.agentId}`,
    );
  });

  return {
    httpServer,
    frontend,
    agents,
    close: () =>
      new Promise((resolve, reject) => {
        wss.close();
        httpServer.close((err) => (err ? reject(err) : resolve()));
      }),
  };
}

/** Connect scheduler tick completion to frontend snapshot + agent observations. */
export function wireTickBroadcasts(
  scheduler: TickScheduler,
  frontend: FrontendGateway,
  agents: AgentGateway,
): void {
  scheduler.setOnTickComplete(({ tick }) => {
    void tick;
    frontend.broadcastSnapshot(scheduler.world.getSnapshot());
    agents.broadcastObservations();
  });
}
