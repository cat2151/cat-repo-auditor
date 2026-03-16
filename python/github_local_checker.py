#!/usr/bin/env python3
"""Backward-compatible entrypoint for github_local_checker."""

import importlib
import sys
from pathlib import Path


def main() -> None:
    project_root = Path(__file__).resolve().parent
    src_path = project_root / "src"
    src_path_str = str(src_path)
    if src_path_str not in sys.path:
        sys.path.insert(0, src_path_str)

    app_module = importlib.import_module("github_local_checker.app")
    app_main = getattr(app_module, "main", None)
    if not callable(app_main):
        raise RuntimeError("github_local_checker.app.main() was not found or is not callable")

    app_main()


if __name__ == "__main__":
    main()
