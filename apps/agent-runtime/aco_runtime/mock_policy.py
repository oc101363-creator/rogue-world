"""Rule-based mock policy: harvest on ore, else walk toward nearest resource."""

from __future__ import annotations

from typing import Any

from .protocol import harvest_action, idle_action, move_action


def _resource_entities(obs: dict[str, Any]) -> list[dict[str, Any]]:
    visible = obs.get("visible") or {}
    entities = visible.get("entities") or []
    resources: list[dict[str, Any]] = []
    for ent in entities:
        if not isinstance(ent, dict):
            continue
        if ent.get("type") != "resource":
            continue
        ore = ent.get("ore", 0)
        if ore is None or ore <= 0:
            continue
        resources.append(ent)
    return resources


def _manhattan(ax: int, ay: int, bx: int, by: int) -> int:
    return abs(ax - bx) + abs(ay - by)


def _nearest_resource(
    self_x: int,
    self_y: int,
    resources: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if not resources:
        return None
    # Stable: min by (distance, x, y, id) so ties are deterministic
    return min(
        resources,
        key=lambda r: (
            _manhattan(self_x, self_y, int(r["x"]), int(r["y"])),
            int(r["x"]),
            int(r["y"]),
            str(r.get("id", "")),
        ),
    )


def _step_toward(self_x: int, self_y: int, tx: int, ty: int) -> dict[str, Any]:
    """One 4-way step toward target. Prefer larger axis delta; tie → prefer dx."""
    dx = tx - self_x
    dy = ty - self_y
    if dx == 0 and dy == 0:
        return idle_action()

    abs_dx = abs(dx)
    abs_dy = abs(dy)

    if abs_dx > abs_dy:
        step_dx = 1 if dx > 0 else -1
        return move_action(step_dx, 0)
    if abs_dy > abs_dx:
        step_dy = 1 if dy > 0 else -1
        return move_action(0, step_dy)
    # Tie: prefer dx
    step_dx = 1 if dx > 0 else -1
    return move_action(step_dx, 0)


def decide(obs: dict[str, Any]) -> list[dict[str, Any]]:
    """
    Decide actions from an observation.

    Rules:
    - If agent stands on a resource entity with ore > 0 → harvest
    - Else move one step toward nearest such resource (Manhattan, 4-way)
    - Else idle
    """
    self_info = obs.get("self") or {}
    self_x = int(self_info.get("x", 0))
    self_y = int(self_info.get("y", 0))

    resources = _resource_entities(obs)

    # Standing on a resource with ore
    for ent in resources:
        if int(ent["x"]) == self_x and int(ent["y"]) == self_y:
            return [harvest_action()]

    nearest = _nearest_resource(self_x, self_y, resources)
    if nearest is None:
        return [idle_action()]

    return [_step_toward(self_x, self_y, int(nearest["x"]), int(nearest["y"]))]
