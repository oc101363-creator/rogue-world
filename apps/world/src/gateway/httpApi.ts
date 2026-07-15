import type { IncomingMessage, ServerResponse } from "node:http";
import { URL } from "node:url";
import type { Action, ActionType } from "@aco/protocol";
import { ALLOWED_ACTIONS, PROTOCOL_VERSION } from "@aco/protocol";
import type { World } from "../core/world.js";
import type { TickScheduler } from "../scheduler/tick.js";
import type { FrontendGateway } from "./frontendGateway.js";
import { config } from "../config.js";
import { tilesAsStrings } from "../map/grid.js";

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c) => chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c)));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

function sendJson(
  res: ServerResponse,
  status: number,
  body: unknown,
): void {
  const data = JSON.stringify(body, null, 0);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type",
  });
  res.end(data);
}

function sendText(res: ServerResponse, status: number, text: string): void {
  res.writeHead(status, {
    "Content-Type": "text/plain; charset=utf-8",
    "Access-Control-Allow-Origin": "*",
  });
  res.end(text);
}

function parseActionFromBody(body: Record<string, unknown>): {
  ok: true;
  action: Action;
} | {
  ok: false;
  error: string;
} {
  const type = body.type ?? body.action;
  if (typeof type !== "string") {
    return { ok: false, error: "missing action type" };
  }
  if (!(ALLOWED_ACTIONS as string[]).includes(type)) {
    return {
      ok: false,
      error: `unknown action type: ${type}; allowed: ${ALLOWED_ACTIONS.join(",")}`,
    };
  }

  const payload =
    body.payload && typeof body.payload === "object" && !Array.isArray(body.payload)
      ? (body.payload as Record<string, unknown>)
      : body;

  switch (type as ActionType) {
    case "move": {
      const dx = Number(payload.dx);
      const dy = Number(payload.dy);
      if (![-1, 0, 1].includes(dx) || ![-1, 0, 1].includes(dy)) {
        return { ok: false, error: "move requires dx,dy in -1|0|1" };
      }
      if (Math.abs(dx) + Math.abs(dy) !== 1) {
        return { ok: false, error: "move must be 4-way (|dx|+|dy|==1)" };
      }
      return {
        ok: true,
        action: {
          type: "move",
          payload: { dx: dx as -1 | 0 | 1, dy: dy as -1 | 0 | 1 },
        },
      };
    }
    case "harvest":
      return { ok: true, action: { type: "harvest", payload: {} } };
    case "idle":
      return { ok: true, action: { type: "idle", payload: {} } };
    case "say": {
      const text = String(payload.text ?? "");
      if (!text) return { ok: false, error: "say requires text" };
      if (text.length > config.maxSayLength) {
        return { ok: false, error: `say text max ${config.maxSayLength}` };
      }
      return { ok: true, action: { type: "say", payload: { text } } };
    }
    default:
      return { ok: false, error: `unsupported action: ${type}` };
  }
}

function renderAscii(world: World): string {
  const tiles = tilesAsStrings(world.grid).map((row) => row.split(""));
  for (const e of world.store.all()) {
    const p = e.components.position;
    if (!p) continue;
    if (p.y < 0 || p.y >= tiles.length) continue;
    if (p.x < 0 || p.x >= (tiles[p.y]?.length ?? 0)) continue;
    const glyph =
      e.type === "agent" && world.focusedAgentId === e.id
        ? "@"
        : (e.components.appearance?.glyph ?? "?");
    tiles[p.y]![p.x] = glyph;
  }
  return tiles.map((r) => r.join("")).join("\n") + "\n";
}

export function createHttpHandler(
  world: World,
  scheduler: TickScheduler,
  frontend: FrontendGateway,
): (req: IncomingMessage, res: ServerResponse) => void {
  return (req, res) => {
    void handle(req, res, world, scheduler, frontend);
  };
}

async function handle(
  req: IncomingMessage,
  res: ServerResponse,
  world: World,
  scheduler: TickScheduler,
  frontend: FrontendGateway,
): Promise<void> {
  const host = req.headers.host ?? "127.0.0.1";
  const url = new URL(req.url ?? "/", `http://${host}`);
  const path = url.pathname;
  const method = (req.method ?? "GET").toUpperCase();

  if (method === "OPTIONS") {
    res.writeHead(204, {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    });
    res.end();
    return;
  }

  try {
    // Health / root
    if (path === "/" || path === "/health") {
      sendJson(res, 200, {
        ok: true,
        service: "aco-world",
        protocolVersion: PROTOCOL_VERSION,
        tick: world.tick,
        decisionTick: scheduler.decisionTick,
      });
      return;
    }

    // GET /api/status
    if (method === "GET" && path === "/api/status") {
      const agents = world.getAgentIds().map((id) => {
        const pos = world.store.getPosition(id);
        const inv = world.store.get(id)?.components.inventory;
        return {
          id,
          x: pos?.x ?? null,
          y: pos?.y ?? null,
          inventory: inv ? { ore: inv.ore } : { ore: 0 },
          focused: world.focusedAgentId === id,
        };
      });
      sendJson(res, 200, {
        ok: true,
        protocolVersion: PROTOCOL_VERSION,
        tick: world.tick,
        decisionTick: scheduler.decisionTick,
        map: { width: world.grid.width, height: world.grid.height },
        focusedAgentId: world.focusedAgentId,
        agents,
        allowed_actions: [...ALLOWED_ACTIONS],
      });
      return;
    }

    // GET /api/observe?agentId=
    if (method === "GET" && path === "/api/observe") {
      const agentId = url.searchParams.get("agentId") ?? config.agentId;
      if (!world.store.has(agentId)) {
        sendJson(res, 404, { ok: false, error: `unknown agent: ${agentId}` });
        return;
      }
      const obs = world.buildObservation(agentId);
      obs.tick = scheduler.decisionTick;
      sendJson(res, 200, obs);
      return;
    }

    // GET /api/snapshot
    if (method === "GET" && path === "/api/snapshot") {
      sendJson(res, 200, world.getSnapshot());
      return;
    }

    // GET /api/map  (ASCII text or JSON)
    if (method === "GET" && path === "/api/map") {
      const format = url.searchParams.get("format") ?? "text";
      const ascii = renderAscii(world);
      if (format === "json") {
        sendJson(res, 200, {
          tick: world.tick,
          decisionTick: scheduler.decisionTick,
          map: ascii,
          width: world.grid.width,
          height: world.grid.height,
        });
        return;
      }
      sendText(res, 200, ascii);
      return;
    }

    // GET /api/events?last=N
    if (method === "GET" && path === "/api/events") {
      const last = Math.min(
        200,
        Math.max(1, Number(url.searchParams.get("last") ?? 20) || 20),
      );
      sendJson(res, 200, {
        tick: world.tick,
        events: world.bus.recent(last),
      });
      return;
    }

    // POST /api/focus  { agentId }
    if (method === "POST" && path === "/api/focus") {
      const raw = await readBody(req);
      let body: Record<string, unknown> = {};
      if (raw.trim()) {
        try {
          body = JSON.parse(raw) as Record<string, unknown>;
        } catch {
          sendJson(res, 400, { ok: false, error: "invalid_json" });
          return;
        }
      }
      const agentId =
        (typeof body.agentId === "string" && body.agentId) ||
        url.searchParams.get("agentId");
      if (!agentId) {
        sendJson(res, 400, { ok: false, error: "agentId required" });
        return;
      }
      if (!world.store.has(agentId)) {
        sendJson(res, 404, { ok: false, error: `unknown agent: ${agentId}` });
        return;
      }
      world.setFocusedAgent(agentId);
      frontend.broadcastSnapshot(world.getSnapshot());
      sendJson(res, 200, {
        ok: true,
        focusedAgentId: world.focusedAgentId,
        tick: world.tick,
      });
      return;
    }

    // POST /api/act  { agentId?, type|action, dx?, dy?, text?, tick? }
    if (method === "POST" && path === "/api/act") {
      const raw = await readBody(req);
      let body: Record<string, unknown> = {};
      if (raw.trim()) {
        try {
          body = JSON.parse(raw) as Record<string, unknown>;
        } catch {
          sendJson(res, 400, { ok: false, error: "invalid_json" });
          return;
        }
      }

      const agentId =
        (typeof body.agentId === "string" && body.agentId) ||
        url.searchParams.get("agentId") ||
        config.agentId;

      if (!world.store.has(agentId)) {
        sendJson(res, 404, { ok: false, error: `unknown agent: ${agentId}` });
        return;
      }

      const parsed = parseActionFromBody(body);
      if (!parsed.ok) {
        sendJson(res, 400, { ok: false, error: parsed.error });
        return;
      }

      const tick =
        typeof body.tick === "number" && Number.isInteger(body.tick)
          ? body.tick
          : scheduler.decisionTick;

      const result = scheduler.submitActionBatch({
        agentId,
        tick,
        actions: [parsed.action],
      });

      if (!result.ok) {
        sendJson(res, 409, {
          ok: false,
          error: result.reason ?? "rejected",
          decisionTick: scheduler.decisionTick,
          submittedTick: tick,
        });
        return;
      }

      sendJson(res, 200, {
        ok: true,
        queued: true,
        agentId,
        tick,
        decisionTick: scheduler.decisionTick,
        action: parsed.action,
      });
      return;
    }

    // POST /api/act/batch  { agentId?, tick?, actions: Action[] }
    if (method === "POST" && path === "/api/act/batch") {
      const raw = await readBody(req);
      let body: Record<string, unknown> = {};
      try {
        body = JSON.parse(raw || "{}") as Record<string, unknown>;
      } catch {
        sendJson(res, 400, { ok: false, error: "invalid_json" });
        return;
      }
      const agentId =
        (typeof body.agentId === "string" && body.agentId) || config.agentId;
      if (!world.store.has(agentId)) {
        sendJson(res, 404, { ok: false, error: `unknown agent: ${agentId}` });
        return;
      }
      if (!Array.isArray(body.actions)) {
        sendJson(res, 400, { ok: false, error: "actions must be array" });
        return;
      }
      const tick =
        typeof body.tick === "number" && Number.isInteger(body.tick)
          ? body.tick
          : scheduler.decisionTick;
      const result = scheduler.submitActionBatch({
        agentId,
        tick,
        actions: body.actions as Action[],
      });
      if (!result.ok) {
        sendJson(res, 409, {
          ok: false,
          error: result.reason ?? "rejected",
          decisionTick: scheduler.decisionTick,
        });
        return;
      }
      sendJson(res, 200, {
        ok: true,
        queued: true,
        agentId,
        tick,
        actions: body.actions,
      });
      return;
    }

    sendJson(res, 404, { ok: false, error: `not found: ${method} ${path}` });
  } catch (err) {
    console.error("[http]", err);
    sendJson(res, 500, {
      ok: false,
      error: err instanceof Error ? err.message : "internal_error",
    });
  }
}
