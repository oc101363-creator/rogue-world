"""Lightweight tests for protocol helpers and message handling."""

from __future__ import annotations

import json
import unittest
from typing import Any

from aco_runtime.client import handle_message
from aco_runtime.protocol import (
    PROTOCOL_VERSION,
    action_batch,
    filter_allowed_actions,
    hello_ack,
    idle_action,
    move_action,
)


class TestProtocolBuilders(unittest.TestCase):
    def test_hello_ack(self) -> None:
        msg = hello_ack("mock")
        self.assertEqual(
            msg,
            {
                "type": "hello_ack",
                "protocolVersion": PROTOCOL_VERSION,
                "runtime": "mock",
            },
        )

    def test_action_batch(self) -> None:
        msg = action_batch("agent-1", 12, [move_action(1, 0)])
        self.assertEqual(msg["type"], "action_batch")
        self.assertEqual(msg["agentId"], "agent-1")
        self.assertEqual(msg["tick"], 12)
        self.assertEqual(msg["actions"][0]["type"], "move")

    def test_filter_allowed_drops_unknown(self) -> None:
        actions = [
            {"type": "attack", "payload": {}},
            {"type": "move", "payload": {"dx": 1, "dy": 0}},
        ]
        filtered = filter_allowed_actions(actions, ["move", "idle"])
        self.assertEqual(len(filtered), 1)
        self.assertEqual(filtered[0]["type"], "move")

    def test_filter_empty_becomes_idle(self) -> None:
        self.assertEqual(
            filter_allowed_actions([{"type": "attack", "payload": {}}], ["idle"]),
            [idle_action()],
        )


class TestHandleMessage(unittest.IsolatedAsyncioTestCase):
    async def test_hello_sends_ack(self) -> None:
        sent: list[dict[str, Any]] = []

        async def send(msg: dict[str, Any]) -> None:
            sent.append(msg)

        await handle_message(
            json.dumps(
                {
                    "type": "hello",
                    "protocolVersion": "1.0",
                    "agentId": "agent-1",
                    "map": {"width": 16, "height": 12},
                }
            ),
            mode="mock",
            decide=lambda _obs: [idle_action()],
            send=send,
        )
        self.assertEqual(len(sent), 1)
        self.assertEqual(sent[0]["type"], "hello_ack")
        self.assertEqual(sent[0]["runtime"], "mock")

    async def test_observation_sends_action_batch(self) -> None:
        sent: list[dict[str, Any]] = []

        async def send(msg: dict[str, Any]) -> None:
            sent.append(msg)

        def decide(_obs: dict[str, Any]) -> list[dict[str, Any]]:
            return [{"type": "harvest", "payload": {}}]

        obs = {
            "type": "observation",
            "tick": 7,
            "self": {"id": "agent-1", "x": 1, "y": 1, "inventory": {"ore": 0}},
            "visible": {"width": 4, "height": 4, "tiles": [], "entities": []},
            "events": [],
            "allowed_actions": ["move", "harvest", "idle", "say"],
            "focused": False,
            "goal": None,
        }
        await handle_message(
            json.dumps(obs),
            mode="mock",
            decide=decide,
            send=send,
        )
        self.assertEqual(len(sent), 1)
        self.assertEqual(sent[0]["type"], "action_batch")
        self.assertEqual(sent[0]["agentId"], "agent-1")
        self.assertEqual(sent[0]["tick"], 7)
        self.assertEqual(sent[0]["actions"], [{"type": "harvest", "payload": {}}])

    async def test_ping_sends_pong(self) -> None:
        sent: list[dict[str, Any]] = []

        async def send(msg: dict[str, Any]) -> None:
            sent.append(msg)

        await handle_message(
            json.dumps({"type": "ping", "id": "k1"}),
            mode="mock",
            decide=lambda _o: [idle_action()],
            send=send,
        )
        self.assertEqual(sent, [{"type": "pong", "id": "k1"}])


if __name__ == "__main__":
    unittest.main()
