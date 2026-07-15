"""Optional LLM policy via OpenAI-compatible chat completions (stdlib urllib)."""

from __future__ import annotations

import json
import logging
import os
import urllib.error
import urllib.request
from typing import Any

from .protocol import filter_allowed_actions, idle_action

logger = logging.getLogger(__name__)

DEFAULT_BASE_URL = "https://api.openai.com/v1"
DEFAULT_MODEL = "gpt-4o-mini"

SYSTEM_PROMPT = """You are an agent in a grid world simulation (Agent Civilization OS).
You receive an observation JSON each tick and must respond with ONLY a JSON object:
{"actions":[...]}

Each action is one of (respect allowed_actions from the observation):
- {"type":"move","payload":{"dx":-1|0|1,"dy":-1|0|1}} with |dx|+|dy|==1 (4-way only)
- {"type":"harvest","payload":{}}
- {"type":"idle","payload":{}}
- {"type":"say","payload":{"text":"..."}}

Rules:
- Prefer harvesting when standing on a resource with ore > 0.
- Otherwise move toward ore or idle.
- At most one primary mutating action (move/harvest/idle); say may accompany.
- Respond with raw JSON only — no markdown fences, no commentary.
"""


def _config() -> tuple[str | None, str, str]:
    api_key = os.environ.get("ACO_LLM_API_KEY") or None
    base_url = (os.environ.get("ACO_LLM_BASE_URL") or DEFAULT_BASE_URL).rstrip("/")
    model = os.environ.get("ACO_LLM_MODEL") or DEFAULT_MODEL
    return api_key, base_url, model


def _extract_json_object(text: str) -> dict[str, Any]:
    text = text.strip()
    if text.startswith("```"):
        # Strip optional markdown fence
        lines = text.splitlines()
        if lines and lines[0].startswith("```"):
            lines = lines[1:]
        if lines and lines[-1].strip() == "```":
            lines = lines[:-1]
        text = "\n".join(lines).strip()
    return json.loads(text)


def _chat_completion(
    api_key: str,
    base_url: str,
    model: str,
    observation: dict[str, Any],
    timeout: float = 15.0,
) -> str:
    url = f"{base_url}/chat/completions"
    body = {
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {
                "role": "user",
                "content": json.dumps(observation, ensure_ascii=False),
            },
        ],
        "temperature": 0,
    }
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        payload = json.loads(resp.read().decode("utf-8"))
    choices = payload.get("choices") or []
    if not choices:
        raise ValueError("LLM response missing choices")
    message = choices[0].get("message") or {}
    content = message.get("content")
    if not isinstance(content, str) or not content.strip():
        raise ValueError("LLM response missing message content")
    return content


def decide(obs: dict[str, Any]) -> list[dict[str, Any]]:
    """
    Ask the configured LLM for actions. On any failure return idle.
    """
    api_key, base_url, model = _config()
    if not api_key:
        logger.warning("ACO_LLM_API_KEY not set; falling back to idle")
        return [idle_action()]

    try:
        content = _chat_completion(api_key, base_url, model, obs)
        parsed = _extract_json_object(content)
        actions = parsed.get("actions")
        if not isinstance(actions, list):
            raise ValueError("response JSON missing 'actions' list")
        allowed = obs.get("allowed_actions")
        if allowed is not None and not isinstance(allowed, list):
            allowed = None
        return filter_allowed_actions(actions, allowed)
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError) as exc:
        logger.warning("LLM request failed: %s", exc)
        return [idle_action()]
    except (json.JSONDecodeError, ValueError, KeyError, TypeError) as exc:
        logger.warning("LLM response parse/validation failed: %s", exc)
        return [idle_action()]
    except Exception as exc:  # noqa: BLE001 — policy must never crash the runtime
        logger.warning("LLM policy unexpected error: %s", exc)
        return [idle_action()]
