import fs from "node:fs";
import path from "node:path";
import Database from "better-sqlite3";
import type { WorldEvent } from "@aco/protocol";

export class EventLogDb {
  private db: Database.Database;

  constructor(dbPath: string) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });
    this.db = new Database(dbPath);
    this.db.pragma("journal_mode = WAL");
    this.migrate();
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        tick INTEGER NOT NULL,
        type TEXT NOT NULL,
        payload TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_events_tick ON events(tick);

      CREATE TABLE IF NOT EXISTS snapshots (
        tick INTEGER PRIMARY KEY,
        state_json TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
    `);
  }

  insertEvent(event: WorldEvent): void {
    const stmt = this.db.prepare(
      `INSERT INTO events (tick, type, payload, created_at) VALUES (?, ?, ?, ?)`,
    );
    stmt.run(
      event.tick ?? 0,
      event.type,
      JSON.stringify(event.payload ?? {}),
      new Date().toISOString(),
    );
  }

  insertEvents(events: WorldEvent[]): void {
    const insert = this.db.prepare(
      `INSERT INTO events (tick, type, payload, created_at) VALUES (?, ?, ?, ?)`,
    );
    const tx = this.db.transaction((rows: WorldEvent[]) => {
      const now = new Date().toISOString();
      for (const event of rows) {
        insert.run(
          event.tick ?? 0,
          event.type,
          JSON.stringify(event.payload ?? {}),
          now,
        );
      }
    });
    tx(events);
  }

  insertSnapshot(tick: number, state: unknown): void {
    this.db
      .prepare(
        `INSERT OR REPLACE INTO snapshots (tick, state_json, created_at) VALUES (?, ?, ?)`,
      )
      .run(tick, JSON.stringify(state), new Date().toISOString());
  }

  countEvents(): number {
    const row = this.db.prepare(`SELECT COUNT(*) as c FROM events`).get() as {
      c: number;
    };
    return row.c;
  }

  close(): void {
    this.db.close();
  }
}
