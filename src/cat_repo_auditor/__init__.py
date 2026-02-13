"""cat_repo_auditor package."""

from .config import DEFAULT_CONFIG_TEXT, ConfigError, load_config, write_default_config

__all__ = [
    "DEFAULT_CONFIG_TEXT",
    "ConfigError",
    "load_config",
    "write_default_config",
]
