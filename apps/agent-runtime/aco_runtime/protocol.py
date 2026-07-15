"""Protocol helpers and message builders for ACO agent runtime."""

from __future__ import annotations

from typing import Any

PROTOCOL_VERSION = "1.0"
ALLOWED_ACTIONS = ("move", "harvest", "idle", "say")


def hello_ack(runtime: str) -> dict[str, Any]:
    """Build a hello_ack message for the World server."""
    if runtime not in ("mock", "llm"):
        raise ValueError(f"runtime must be 'mock' or 'llm', got {runtime!r}")
    return {
        "type": "hello_ack",
        "protocolVersion": PROTOCOL_VERSION,
        "runtime": runtime,
    }


def action_batch(
    agent_id: str,
    tick: int,
    actions: list[dict[str, Any]],
    *,
    msg_id: str | None = None,
) -> dict[str, Any]:
    """Build an action_batch message."""
    msg: dict[str, Any] = {
        "type": "action_batch",
        "agentId": agent_id,
        "tick": tick,
        "actions": actions,
    }
    if msg_id is not None:
        msg["id"] = msg_id
    return msg


def pong(msg_id: str | None = None) -> dict[str, Any]:
    """Build a pong keepalive response."""
    msg: dict[str, Any] = {"type": "pong"}
    if msg_id is not None:
        msg["id"] = msg_id
    return msg


def idle_action() -> dict[str, Any]:
    return {"type": "idle", "payload": {}}


def harvest_action() -> dict[str, Any]:
    return {"type": "harvest", "payload": {}}


def move_action(dx: int, dy: int) -> dict[str, Any]:
    if abs(dx) + abs(dy) != 1 or dx not in (-1, 0, 1) or dy not in (-1, 0, 1):
        raise ValueError(f"move must be 4-way unit step, got dx={dx}, dy={dy}")
    return {"type": "move", "payload": {"dx": dx, "dy": dy}}


def filter_allowed_actions(
    actions: list[dict[str, Any]],
    allowed: list[str] | None,
) -> list[dict[str, Any]]:
    """Drop actions whose type is not in allowed_actions; empty → idle."""
    if not allowed:
        allowed_set = set(ALLOWED_ACTIONS)
    else:
        allowed_set = set(allowed)
    filtered = [a for a in actions if isinstance(a, dict) and a.get("type") in allowed_set]
    if not filtered:
        return [idle_action()]
    return filtered
