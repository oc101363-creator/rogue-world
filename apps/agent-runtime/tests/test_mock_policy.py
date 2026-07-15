"""Unit tests for mock_policy.decide."""

from __future__ import annotations

import copy
import unittest

from aco_runtime.mock_policy import decide


def _base_obs(
    *,
    x: int = 3,
    y: int = 4,
    entities: list | None = None,
) -> dict:
    return {
        "type": "observation",
        "protocolVersion": "1.0",
        "tick": 12,
        "self": {
            "id": "agent-1",
            "x": x,
            "y": y,
            "inventory": {"ore": 0},
        },
        "visible": {
            "width": 16,
            "height": 12,
            "tiles": [],
            "entities": entities if entities is not None else [],
        },
        "events": [],
        "allowed_actions": ["move", "harvest", "idle", "say"],
        "focused": True,
        "goal": None,
    }


class TestMockPolicyHarvest(unittest.TestCase):
    def test_harvest_when_standing_on_resource_with_ore(self) -> None:
        obs = _base_obs(
            x=8,
            y=2,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 8,
                    "y": 2,
                    "ore": 10,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(actions, [{"type": "harvest", "payload": {}}])

    def test_no_harvest_when_ore_is_zero(self) -> None:
        obs = _base_obs(
            x=8,
            y=2,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 8,
                    "y": 2,
                    "ore": 0,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(actions, [{"type": "idle", "payload": {}}])


class TestMockPolicyMove(unittest.TestCase):
    def test_move_toward_ore_prefer_larger_axis(self) -> None:
        # Agent at (3,4), ore at (8,2): dx=5, dy=-2 → prefer dx
        obs = _base_obs(
            x=3,
            y=4,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 8,
                    "y": 2,
                    "ore": 10,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(
            actions,
            [{"type": "move", "payload": {"dx": 1, "dy": 0}}],
        )

    def test_move_toward_ore_prefer_dy_when_larger(self) -> None:
        # Agent at (5,8), ore at (5,2): dx=0, dy=-6 → move up
        obs = _base_obs(
            x=5,
            y=8,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 5,
                    "y": 2,
                    "ore": 3,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(
            actions,
            [{"type": "move", "payload": {"dx": 0, "dy": -1}}],
        )

    def test_tie_prefers_dx(self) -> None:
        # Equal |dx| and |dy|: prefer dx
        obs = _base_obs(
            x=3,
            y=3,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 5,
                    "y": 5,
                    "ore": 1,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(
            actions,
            [{"type": "move", "payload": {"dx": 1, "dy": 0}}],
        )

    def test_move_left_and_down(self) -> None:
        obs = _base_obs(
            x=10,
            y=2,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 2,
                    "y": 8,
                    "ore": 5,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        # |dx|=8 > |dy|=6 → move left
        self.assertEqual(
            actions,
            [{"type": "move", "payload": {"dx": -1, "dy": 0}}],
        )

    def test_nearest_resource_chosen(self) -> None:
        obs = _base_obs(
            x=4,
            y=4,
            entities=[
                {
                    "id": "far",
                    "type": "resource",
                    "x": 14,
                    "y": 10,
                    "ore": 10,
                    "glyph": "M",
                },
                {
                    "id": "near",
                    "type": "resource",
                    "x": 5,
                    "y": 4,
                    "ore": 2,
                    "glyph": "M",
                },
            ],
        )
        actions = decide(obs)
        self.assertEqual(
            actions,
            [{"type": "move", "payload": {"dx": 1, "dy": 0}}],
        )

    def test_never_diagonal(self) -> None:
        obs = _base_obs(
            x=1,
            y=1,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 4,
                    "y": 4,
                    "ore": 1,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(len(actions), 1)
        payload = actions[0]["payload"]
        self.assertEqual(abs(payload["dx"]) + abs(payload["dy"]), 1)
        self.assertIn(payload["dx"], (-1, 0, 1))
        self.assertIn(payload["dy"], (-1, 0, 1))


class TestMockPolicyIdle(unittest.TestCase):
    def test_idle_when_no_resources(self) -> None:
        obs = _base_obs(x=3, y=4, entities=[])
        actions = decide(obs)
        self.assertEqual(actions, [{"type": "idle", "payload": {}}])

    def test_idle_when_only_depleted_resources(self) -> None:
        obs = _base_obs(
            x=3,
            y=4,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 8,
                    "y": 2,
                    "ore": 0,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(actions, [{"type": "idle", "payload": {}}])

    def test_ignores_non_resource_entities(self) -> None:
        obs = _base_obs(
            x=3,
            y=4,
            entities=[
                {
                    "id": "agent-2",
                    "type": "agent",
                    "x": 5,
                    "y": 5,
                    "glyph": "A",
                }
            ],
        )
        actions = decide(obs)
        self.assertEqual(actions, [{"type": "idle", "payload": {}}])


class TestMockPolicyFixture(unittest.TestCase):
    def test_matches_protocol_fixture_shape(self) -> None:
        # Same geometry as packages/protocol/fixtures/observation.example.json
        obs = _base_obs(
            x=3,
            y=4,
            entities=[
                {
                    "id": "ore-1",
                    "type": "resource",
                    "x": 8,
                    "y": 2,
                    "ore": 10,
                    "glyph": "M",
                }
            ],
        )
        actions = decide(copy.deepcopy(obs))
        self.assertEqual(actions[0]["type"], "move")
        self.assertEqual(actions[0]["payload"], {"dx": 1, "dy": 0})


if __name__ == "__main__":
    unittest.main()
