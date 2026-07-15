import type { WorldEvent } from "@aco/protocol";

export type { WorldEvent };

export function makeEvent(
  type: string,
  payload: Record<string, unknown>,
  tick?: number,
): WorldEvent {
  const event: WorldEvent = { type, payload };
  if (tick !== undefined) event.tick = tick;
  return event;
}
