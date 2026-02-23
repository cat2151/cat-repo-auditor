# cat-repo-auditor

A CLI tool to bulk-check the maintenance status of GitHub repositories.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Overview

It fetches the 20 most recent repositories of a user authenticated with the `gh` command (GitHub CLI), and automatically checks the following items for each repository. The results are output to a JSON file, and a colorized summary is displayed in the terminal.

### Check Items

| Item | Description |
|------|------|
| `README.ja.md` | Presence of a Japanese README |
| DeepWiki Entry | Whether there is a DeepWiki link within `README.ja.md` |
| `google*.html` | Presence of verification files for Google Search Console |
| `AGENTS.md` / `copilot-instructions.md` | Presence of instruction files for AI agents |
| `.github/workflows/*.yml` | Presence of CI/CD workflows |
| `_config.yml` | Presence of a Jekyll configuration file |

## Requirements

- Python 3.11 or higher (or Python 3.10 or lower + `pip install tomli`)
- [GitHub CLI](https://cli.github.com/) installed and authenticated with `gh auth login`

## Installation

```bash
git clone https://github.com/cat2151/cat-repo-auditor.git
cd cat-repo-auditor
```

No additional packages are required (uses only Python 3.11+ standard library).

For Python 3.10 or lower:

```bash
pip install tomli
```

## Configuration

Create `config.toml` in the current directory.

```toml
github_user = "your-github-username"
```

## Usage

```bash
python cat_repo_auditor.py
```

Options:

```
--output, -o    JSON output file path (default: repo_analysis.json)
--config, -c    Configuration file path (default: config.toml)
```

## Example Output

A summary is displayed in Monokai colors in the terminal.

```
=== GitHub Repository Analysis CLI ===
Execution Time: 2026-02-23 12:00:00
Target User: your-github-username
Authentication: Obtained via gh auth token

[1/3] Fetching repositories for your-github-username...
      20 items fetched

[2/3] Analyzing each repository...
  [ 1/20] my-project
         ✓ README.ja | ✗ DeepWiki | ✗ google | ✓ agents | ✓ CI | ✗ jekyll

[3/3] Summary
======================================================================
  README.ja.md  [15/20 present / 5/20 missing]
    ✗ some-repo
      https://github.com/your-github-username/some-repo
    ...
```

The JSON file (`repo_analysis.json`) contains detailed information for each repository.

## github_local_checker.py

- A checking tool centered on the local side
- Uses the same TOML file
- When run normally, it performs a dry-run check of local repositories and prints the results.
- When run with `--pull`, it pulls all pullable repositories.
- Its purpose is for users with many small experimental repositories to easily manage them by pulling them all locally.

## check_local_workflows.py

- A checking tool centered on the local side
- Uses the same TOML file
- Performs hash checks
- Its purpose is for users with many small experimental repositories to easily manage them.

## sync_workflows.py

- Its purpose is to synchronize workflow files across local repositories to match the majority.
- A confirmation prompt (y/[N]) is displayed before the final commit and push.