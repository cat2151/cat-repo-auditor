from pathlib import Path

CONFIG_FILE = Path(__file__).resolve().parents[2] / "config.toml"
PREREQUISITE = "README.ja.md"

CALL_CHECK_LARGE_FILES_WF = Path(".github/workflows/call-check-large-files.yml")
CHECK_LARGE_FILES_CONFIG = Path(".github/check-large-files.toml")
