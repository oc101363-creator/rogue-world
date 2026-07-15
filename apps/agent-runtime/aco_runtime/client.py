"""WebSocket client loop: hello → observations → action_batch."""

from __future__ import annotations

import asyncio
import json
import logging
from typing import Any, Awaitable, Callable, Protocol

from . import llm_policy, mock_policy
from .protocol import action_batch, hello_ack, pong

logger = logging.getLogger(__name__)

DEFAULT_URL = "ws://127.0.0.1:8080/ws/agent?agentId=agent-1"

DecideFn = Callable[[dict[str, Any]], list[dict[str, Any]]]


class _WebSocketLike(Protocol):
    async def send(self, message: str) -> Any: ...
    def __aiter__(self) -> Any: ...


def policy_for_mode(mode: str) -> DecideFn:
    if mode == "llm":
        return llm_policy.decide
    if mode == "mock":
        return mock_policy.decide
    raise ValueError(f"unknown mode: {mode!r}")


async def handle_message(
    raw: str | bytes,
    *,
    mode: str,
    decide: DecideFn,
    send: Callable[[dict[str, Any]], Awaitable[None]],
) -> None:
    """Process one inbound protocol message."""
    if isinstance(raw, bytes):
        raw = raw.decode("utf-8")
    try:
        msg = json.loads(raw)
    except json.JSONDecodeError:
        logger.error("invalid JSON from world: %s", raw[:200])
        return

    if not isinstance(msg, dict):
        logger.error("expected JSON object, got %s", type(msg).__name__)
        return

    msg_type = msg.get("type")

    if msg_type == "hello":
        ack = hello_ack(mode)
        logger.info(
            "hello received agentId=%s map=%s → hello_ack runtime=%s",
            msg.get("agentId"),
            msg.get("map"),
            mode,
        )
        await send(ack)
        return

    if msg_type == "observation":
        tick = msg.get("tick")
        self_info = msg.get("self") or {}
        agent_id = self_info.get("id")
        if agent_id is None or tick is None:
            logger.error("observation missing self.id or tick: %s", msg)
            return
        try:
            actions = decide(msg)
        except Exception:  # noqa: BLE001
            logger.exception("policy decide failed; sending idle")
            actions = [{"type": "idle", "payload": {}}]
        batch = action_batch(str(agent_id), int(tick), actions)
        logger.info(
            "observation tick=%s agent=%s → %s",
            tick,
            agent_id,
            actions,
        )
        await send(batch)
        return

    if msg_type == "ping":
        await send(pong(msg.get("id")))
        return

    if msg_type == "pong":
        logger.debug("pong received")
        return

    if msg_type == "error":
        logger.error("world error: %s", msg)
        return

    logger.warning("unknown message type: %s", msg_type)


async def run_client(
    url: str = DEFAULT_URL,
    mode: str = "mock",
    *,
    decide: DecideFn | None = None,
) -> None:
    """Connect to World and process messages until the socket closes."""
    try:
        import websockets
    except ImportError as exc:  # pragma: no cover
        raise SystemExit(
            "websockets package is required. Install with: pip install -r requirements.txt"
        ) from exc

    if mode not in ("mock", "llm"):
        raise ValueError(f"mode must be 'mock' or 'llm', got {mode!r}")

    decide_fn = decide or policy_for_mode(mode)

    logger.info("connecting to %s mode=%s", url, mode)
    async with websockets.connect(url) as ws:

        async def send(msg: dict[str, Any]) -> None:
            await ws.send(json.dumps(msg, ensure_ascii=False))

        async for raw in ws:
            await handle_message(raw, mode=mode, decide=decide_fn, send=send)

    logger.info("connection closed")


def run_sync(url: str = DEFAULT_URL, mode: str = "mock") -> None:
    """Blocking entry for CLI."""
    asyncio.run(run_client(url=url, mode=mode))
