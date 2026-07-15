"""CLI entry for ACO agent runtime."""

from __future__ import annotations

import argparse
import logging
import os
import sys

from .client import DEFAULT_URL, run_sync


def build_parser() -> argparse.ArgumentParser:
    default_mode = os.environ.get("ACO_RUNTIME_MODE", "mock")
    parser = argparse.ArgumentParser(
        prog="aco_runtime",
        description="ACO V1 Python Agent Runtime (mock or LLM policy)",
    )
    parser.add_argument(
        "--url",
        default=DEFAULT_URL,
        help=f"World agent WebSocket URL (default: {DEFAULT_URL})",
    )
    parser.add_argument(
        "--mode",
        choices=("mock", "llm"),
        default=default_mode if default_mode in ("mock", "llm") else "mock",
        help="Decision policy: mock (rules) or llm (chat completions)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="Enable debug logging",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    logging.basicConfig(
        level=logging.DEBUG if args.verbose else logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )

    if args.mode == "llm" and not os.environ.get("ACO_LLM_API_KEY"):
        logging.getLogger(__name__).warning(
            "mode=llm but ACO_LLM_API_KEY is unset; policy will idle on every tick"
        )

    try:
        run_sync(url=args.url, mode=args.mode)
    except KeyboardInterrupt:
        logging.getLogger(__name__).info("interrupted")
        return 0
    except Exception as exc:  # noqa: BLE001
        logging.getLogger(__name__).error("runtime failed: %s", exc)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
