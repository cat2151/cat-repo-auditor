"""Configuration helpers for cat-repo-auditor."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict

try:
    import tomllib
except ImportError:  # pragma: no cover - Python 3.11+ includes tomllib; fallback to tomli on older versions
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None

DEFAULT_CONFIG_TEXT = """# Repository Auditor Configuration
# Edit this file to change what gets checked. Updates are intended to be picked up automatically by the app.

check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
]

[display]
show_repo_name = true
show_updated_at = true
highlight_missing = true
"""


class ConfigError(Exception):
    """Raised when configuration cannot be loaded."""


def write_default_config(config_path: str | Path = "audit_config.toml") -> Path:
    """
    Write the default configuration file if it does not already exist.

    Args:
        config_path: Path where the configuration should reside.

    Returns:
        The resolved path to the configuration file.
    """
    path = Path(config_path)
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(DEFAULT_CONFIG_TEXT, encoding="utf-8")
    return path


def load_config(config_path: str | Path = "audit_config.toml") -> Dict[str, Any]:
    """
    Load configuration from a TOML file, creating a default file when missing.

    Args:
        config_path: Path to the TOML configuration file.

    Returns:
        Parsed configuration dictionary.

    Raises:
        ConfigError: If TOML support is unavailable or parsing fails.
    """
    if tomllib is None:
        raise ConfigError("TOML support is required to load configuration.")

    path = write_default_config(config_path)

    try:
        with path.open("rb") as stream:
            return tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:  # type: ignore[attr-defined]
        raise ConfigError(f"Failed to load configuration from {path}") from exc
