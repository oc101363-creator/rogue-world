import type { Action } from "@aco/protocol";
import { ALLOWED_ACTIONS } from "@aco/protocol";
import { config } from "../config.js";

export interface ValidatedBatch {
  mutator: Action | null;
  says: Action[];
  rejected: Array<{ action: Action; reason: string }>;
}

function isKnownActionType(type: string): type is Action["type"] {
  return (ALLOWED_ACTIONS as string[]).includes(type);
}

function validateMovePayload(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return "invalid_payload";
  const p = payload as { dx?: unknown; dy?: unknown };
  if (typeof p.dx !== "number" || typeof p.dy !== "number") {
    return "invalid_move_payload";
  }
  if (!Number.isInteger(p.dx) || !Number.isInteger(p.dy)) {
    return "invalid_move_payload";
  }
  if (Math.abs(p.dx) > 1 || Math.abs(p.dy) > 1) {
    return "invalid_move_payload";
  }
  if (Math.abs(p.dx) + Math.abs(p.dy) !== 1) {
    return "invalid_move_delta";
  }
  return null;
}

function validateSayPayload(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return "invalid_payload";
  const p = payload as { text?: unknown };
  if (typeof p.text !== "string") return "invalid_say_payload";
  if (p.text.length === 0) return "empty_say";
  if (p.text.length > config.maxSayLength) return "say_too_long";
  return null;
}

/**
 * Split a batch into at most one mutator (move|harvest|idle) + any says.
 * Extra mutators and unknown actions are rejected (not applied).
 */
export function validateActionBatch(actions: Action[]): ValidatedBatch {
  const result: ValidatedBatch = {
    mutator: null,
    says: [],
    rejected: [],
  };

  if (!Array.isArray(actions)) {
    return result;
  }

  for (const action of actions) {
    if (!action || typeof action !== "object" || typeof action.type !== "string") {
      result.rejected.push({
        action: action as Action,
        reason: "malformed_action",
      });
      continue;
    }

    if (!isKnownActionType(action.type)) {
      result.rejected.push({ action, reason: "unknown_action_type" });
      continue;
    }

    if (action.type === "say") {
      const err = validateSayPayload(action.payload);
      if (err) {
        result.rejected.push({ action, reason: err });
      } else {
        result.says.push(action);
      }
      continue;
    }

    // Mutators: move | harvest | idle
    if (action.type === "move") {
      const err = validateMovePayload(action.payload);
      if (err) {
        result.rejected.push({ action, reason: err });
        continue;
      }
    }

    if (result.mutator !== null) {
      result.rejected.push({ action, reason: "extra_mutator" });
      continue;
    }

    result.mutator = action;
  }

  return result;
}
