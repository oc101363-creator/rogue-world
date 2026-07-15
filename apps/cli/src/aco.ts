#!/usr/bin/env node
/**
 * aco — thin CLI client for ACO World HTTP API.
 * Agents play the world by shelling out to these commands (see skills/aco-play).
 */

const DEFAULT_BASE = process.env.ACO_WORLD_URL ?? "http://127.0.0.1:8080";
const DEFAULT_AGENT = process.env.ACO_AGENT_ID ?? "agent-1";

function usage(): never {
  console.log(`aco — Agent Civilization OS CLI

Usage:
  aco status
  aco observe [--agent <id>]
  aco map
  aco events [--last N]
  aco focus <agentId>
  aco act move --dx <n> --dy <n> [--agent <id>] [--tick <n>]
  aco act harvest [--agent <id>] [--tick <n>]
  aco act idle [--agent <id>] [--tick <n>]
  aco act say --text <string> [--agent <id>] [--tick <n>]

Env:
  ACO_WORLD_URL   default ${DEFAULT_BASE}
  ACO_AGENT_ID    default ${DEFAULT_AGENT}
`);
  process.exit(1);
}

function baseUrl(): string {
  return (process.env.ACO_WORLD_URL ?? DEFAULT_BASE).replace(/\/$/, "");
}

function parseArgs(argv: string[]) {
  const flags: Record<string, string | boolean> = {};
  const positionals: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--") continue;
    if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq !== -1) {
        flags[a.slice(2, eq)] = a.slice(eq + 1);
        continue;
      }
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        flags[key] = next;
        i++;
      } else {
        flags[key] = true;
      }
    } else {
      positionals.push(a);
    }
  }
  return { flags, positionals };
}

async function request(
  method: string,
  path: string,
  body?: unknown,
): Promise<{ status: number; json: unknown; text: string }> {
  const url = `${baseUrl()}${path}`;
  const init: RequestInit = {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  };
  let res: Response;
  try {
    res = await fetch(url, init);
  } catch (err) {
    console.error(
      JSON.stringify({
        ok: false,
        error: "connection_failed",
        url,
        detail: err instanceof Error ? err.message : String(err),
      }),
    );
    process.exit(2);
  }
  const text = await res.text();
  let json: unknown = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text };
  }
  return { status: res.status, json, text };
}

function printJson(data: unknown, status: number): void {
  console.log(JSON.stringify(data, null, 2));
  if (status >= 400) process.exit(1);
}

async function cmdStatus(): Promise<void> {
  const { status, json } = await request("GET", "/api/status");
  printJson(json, status);
}

async function cmdObserve(agent: string): Promise<void> {
  const q = new URLSearchParams({ agentId: agent });
  const { status, json } = await request("GET", `/api/observe?${q}`);
  printJson(json, status);
}

async function cmdMap(): Promise<void> {
  const { status, text, json } = await request("GET", "/api/map");
  if (status >= 400) {
    printJson(json, status);
    return;
  }
  // Prefer plain ASCII for agents reading the terminal
  process.stdout.write(typeof json === "object" && json && "raw" in (json as object)
    ? String((json as { raw: string }).raw)
    : text.endsWith("\n")
      ? text
      : text + "\n");
}

async function cmdEvents(last: number): Promise<void> {
  const { status, json } = await request("GET", `/api/events?last=${last}`);
  printJson(json, status);
}

async function cmdFocus(agentId: string): Promise<void> {
  const { status, json } = await request("POST", "/api/focus", { agentId });
  printJson(json, status);
}

async function cmdAct(
  actionType: string,
  flags: Record<string, string | boolean>,
): Promise<void> {
  const agent =
    (typeof flags.agent === "string" && flags.agent) || DEFAULT_AGENT;
  const body: Record<string, unknown> = {
    agentId: agent,
    type: actionType,
  };
  if (typeof flags.tick === "string") {
    body.tick = Number(flags.tick);
  }
  if (actionType === "move") {
    if (flags.dx === undefined || flags.dy === undefined) {
      console.error(
        JSON.stringify({ ok: false, error: "move requires --dx and --dy" }),
      );
      process.exit(1);
    }
    body.dx = Number(flags.dx);
    body.dy = Number(flags.dy);
  }
  if (actionType === "say") {
    if (typeof flags.text !== "string") {
      console.error(
        JSON.stringify({ ok: false, error: "say requires --text" }),
      );
      process.exit(1);
    }
    body.text = flags.text;
  }

  const { status, json } = await request("POST", "/api/act", body);
  printJson(json, status);
}

async function main(): Promise<void> {
  // Strip leading "--" inserted by some package runners (pnpm … -- args)
  let argv = process.argv.slice(2);
  while (argv[0] === "--") argv = argv.slice(1);

  if (argv.length === 0 || argv[0] === "-h" || argv[0] === "--help") {
    usage();
  }

  const cmd = argv[0]!;
  const rest = argv.slice(1);
  const { flags, positionals } = parseArgs(rest);

  switch (cmd) {
    case "status":
      await cmdStatus();
      break;
    case "observe": {
      const agent =
        (typeof flags.agent === "string" && flags.agent) || DEFAULT_AGENT;
      await cmdObserve(agent);
      break;
    }
    case "map":
      await cmdMap();
      break;
    case "events": {
      const last =
        typeof flags.last === "string" ? Number(flags.last) || 20 : 20;
      await cmdEvents(last);
      break;
    }
    case "focus": {
      const agentId = positionals[0];
      if (!agentId) {
        console.error(
          JSON.stringify({ ok: false, error: "usage: aco focus <agentId>" }),
        );
        process.exit(1);
      }
      await cmdFocus(agentId);
      break;
    }
    case "act": {
      const actionType = positionals[0];
      if (!actionType) {
        console.error(
          JSON.stringify({
            ok: false,
            error: "usage: aco act <move|harvest|idle|say> ...",
          }),
        );
        process.exit(1);
      }
      await cmdAct(actionType, flags);
      break;
    }
    default:
      console.error(JSON.stringify({ ok: false, error: `unknown command: ${cmd}` }));
      usage();
  }
}

main().catch((err) => {
  console.error(
    JSON.stringify({
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    }),
  );
  process.exit(1);
});
