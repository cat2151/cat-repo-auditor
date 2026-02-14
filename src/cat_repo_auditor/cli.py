"""Command-line entry point for cat-repo-auditor."""

from __future__ import annotations

import argparse
import sys
from typing import Callable, Iterable, Sequence

import requests

from .auditor import AuditResult, GitHubClient, audit_user_repositories
from .auto_updater import maybe_self_update
from .config import ConfigError, load_config


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Audit GitHub repositories for required files.")
    parser.add_argument("--user", required=True, help="GitHub username to audit")
    parser.add_argument("--config", default="audit_config.toml", help="Path to TOML configuration file")
    parser.add_argument("--limit", type=int, default=5, help="Maximum repositories to inspect")
    parser.add_argument("--token", default=None, help="GitHub token (defaults to GITHUB_TOKEN env var)")
    return parser


def _format_table(check_items: Sequence[str], results: Iterable[AuditResult]) -> str:
    columns = ["Repository", "Updated"] + list(check_items)
    widths = {column: len(column) for column in columns}

    normalized_results = list(results)
    for result in normalized_results:
        widths["Repository"] = max(widths["Repository"], len(result.repository))
        widths["Updated"] = max(widths["Updated"], len(result.updated_at or ""))
        for item in check_items:
            mark = "yes" if result.found.get(item) else "no"
            widths[item] = max(widths[item], len(mark))

    def format_row(values: Sequence[str]) -> str:
        return " | ".join(value.ljust(widths[column]) for value, column in zip(values, columns))

    header = format_row(columns)
    divider = "-+-".join("-" * widths[column] for column in columns)
    body_lines = [
        format_row([result.repository, result.updated_at or ""] + [("yes" if result.found.get(item) else "no") for item in check_items])
        for result in normalized_results
    ]

    return "\n".join([header, divider, *body_lines])


def main(
    argv: Sequence[str] | None = None,
    client: GitHubClient | None = None,
    stream=None,
    *,
    self_update: bool = True,
    update_fn: Callable[[], bool] | None = None,
) -> int:
    """
    Run the CLI.

    Args:
        argv: Optional argument list for testing.
        client: Optional GitHubClient override for testing.
        stream: Optional stream to write output to. Defaults to stdout.
        self_update: Whether to perform a self-update check before running.
        update_fn: Optional override for the self-update function.

    Returns:
        Exit code.
    """
    stream = stream or sys.stdout
    args = _build_parser().parse_args(argv)

    if self_update:
        updater = update_fn or maybe_self_update
        try:
            updater()
        except Exception:
            pass

    try:
        config = load_config(args.config)
    except ConfigError as exc:
        stream.write(f"Failed to load configuration: {exc}\n")
        return 1

    check_items = config.get("check_items") or []
    if not check_items:
        stream.write("No check_items configured.\n")
        return 1

    try:
        results = audit_user_repositories(
            args.user,
            check_items,
            limit=args.limit,
            client=client,
            token=args.token,
        )
    except (requests.HTTPError, ValueError) as exc:
        stream.write(f"Audit failed: {exc}\n")
        return 1

    if not results:
        stream.write("No repositories found for the user.\n")
        return 0

    stream.write(_format_table(check_items, results))
    stream.write("\n")
    return 0
