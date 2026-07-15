import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const config = {
  protocolVersion: "1.0" as const,
  mapWidth: 16,
  mapHeight: 12,
  /** Tick interval in ms (2 TPS). */
  tickMs: 500,
  /** Agent decision deadline after observation (ms). */
  decisionTimeoutMs: 2000,
  /** WS listen port. */
  port: 8080,
  /** SQLite path relative to apps/world when run from package root. */
  dbPath: path.resolve(__dirname, "../../data/world.db"),
  /** Snapshot every N ticks. */
  snapshotEveryNTicks: 10,
  /** Initial resource nodes. */
  resourcePositions: [
    { x: 8, y: 2 },
    { x: 5, y: 5 },
    { x: 12, y: 7 },
  ] as const,
  orePerNode: 10,
  agentId: "agent-1",
  /** Agent start: (2, height - 3). */
  agentStart: (height: number) => ({ x: 2, y: height - 3 }),
  maxSayLength: 200,
  recentEventLimit: 50,
};
