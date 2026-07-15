import { config } from "./config.js";
import { createWorld } from "./core/world.js";
import { EventLogDb } from "./persist/db.js";
import { TickScheduler } from "./scheduler/tick.js";
import {
  createWorldServer,
  wireTickBroadcasts,
} from "./gateway/server.js";

async function main(): Promise<void> {
  const world = createWorld();
  const db = new EventLogDb(config.dbPath);

  const scheduler = new TickScheduler({
    world,
    db,
    tickMs: config.tickMs,
    decisionTimeoutMs: config.decisionTimeoutMs,
  });

  const server = createWorldServer(world, scheduler, config.port);
  wireTickBroadcasts(scheduler, server.frontend, server.agents);

  scheduler.start();

  console.log(
    `[world] map ${world.grid.width}x${world.grid.height}, tick ${config.tickMs}ms, db ${config.dbPath}`,
  );
  console.log(
    `[world] agent ${config.agentId} at (${world.store.getPosition(config.agentId)?.x}, ${world.store.getPosition(config.agentId)?.y})`,
  );

  const shutdown = async (signal: string) => {
    console.log(`[world] ${signal}, shutting down…`);
    scheduler.stop();
    try {
      await server.close();
    } catch {
      /* ignore */
    }
    db.close();
    process.exit(0);
  };

  process.on("SIGINT", () => void shutdown("SIGINT"));
  process.on("SIGTERM", () => void shutdown("SIGTERM"));
}

main().catch((err) => {
  console.error("[world] fatal", err);
  process.exit(1);
});
