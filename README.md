# cat-repo-auditor

Current status (2026-02)
- Implemented: configuration helper that creates/loads `audit_config.toml` via `cat_repo_auditor.config.load_config`.
- Implemented: lightweight CLI to audit a GitHub user's repositories for configured files.
- Not yet available: GUI entry point or advanced caching/diffing (planned for the future).

How to verify the current behavior
1. Install Python 3.10+ and dependencies: `python -m pip install -r requirements.txt pytest`
2. Run tests: `pytest`
3. Confirm config generation:
   ```bash
   PYTHONPATH=src python - <<'PY'
   from cat_repo_auditor.config import load_config
   print(load_config())  # creates audit_config.toml if missing
   PY
   ```
4. Run a simple audit (requires network and optionally `GITHUB_TOKEN` for higher rate limits):
   ```bash
   python -m cat_repo_auditor --user <github-username> --limit 5
   # uses audit_config.toml in the current directory by default
   ```

Files of interest
- `src/cat_repo_auditor/config.py`: configuration loader/writer.
- `src/cat_repo_auditor/auditor.py`: GitHub API helper and auditing logic.
- `src/cat_repo_auditor/cli.py`: CLI entry point (`python -m cat_repo_auditor`).
- `audit_config.toml`: default config file (auto-created if absent).

More detailed future plans (currently aspirational) are documented in `README.ja.md`.
