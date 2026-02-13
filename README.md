# cat-repo-auditor

Current status (2026-02)
- Implemented: configuration helper that creates/loads `audit_config.toml` via `cat_repo_auditor.config.load_config`.
- Not yet available: CLI/GUI entry point or repository auditing workflow (planned for the future).
- What you can verify today: configuration file handling through the unit tests and a small import check.

How to verify the current behavior
1. Install Python 3.10+ and dependencies: `python -m pip install -r requirements.txt pytest`
2. Run tests: `pytest`
3. Optionally confirm config generation:
   ```bash
   python - <<'PY'
   from cat_repo_auditor.config import load_config
   print(load_config())  # creates audit_config.toml if missing
   PY
   ```

Files of interest
- `src/cat_repo_auditor/config.py`: configuration loader/writer.
- `audit_config.toml`: default config file (auto-created if absent).

More detailed future plans (currently aspirational) are documented in `README.ja.md`.
