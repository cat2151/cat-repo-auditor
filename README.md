# cat-repo-auditor

A TUI (Terminal User Interface) written in Rust that lists, visualizes, and automates part of the maintenance of GitHub repositories' remote/local status to improve efficiency.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Status

Currently dogfooding.

### Install

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### Run

```
catrepo
```

### Update

```
catrepo update
```

If it fails, run the following (same as the install command):

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### Display the commit hash embedded at build time:

```
catrepo hash
```

### Check for updates:

```
catrepo check
```

### Display CLI help:

```
catrepo help
catrepo --help
```

### Usage

- This is for personal use and not intended for others. The following are also personal notes.

- Motivation behind creation
  - My hobby OSS repositories have grown. Maintaining them is taking up cognitive load and increasing effort.
  - Hence, a TUI. I'm building my own TUI to simplify maintenance.
  - Small-scale TUIs can be easily created with the free version of Claude chat, so I'm going with that approach.

- How to use
  - Config
    - Upon first launch, `config.toml` will be generated in the local config directory, and its full path will be displayed.
      - Use that as a hint to manually edit `config.toml`.
      - It won't work without editing.
  - Help
    - Once launched, press the `?` key to display help.

- PoC
  - Regarding the overall use of the repository, it's a PoC.
  - This is to demonstrate that small-scale TUIs like this can be created with the free version of Claude chat.
  - So, it's also to convey the message: "You should build one for yourself too!"

## The following is old. It's a different project, moved to `python/`. Will be rewritten later.

## Overview

Fetches the 20 most recent repositories of a user authenticated with the `gh` command (GitHub CLI), automatically checks the following items for each repository. The results are output to a JSON file, and a colored summary is displayed in the terminal.

### Check Items

| Item | Description |
|------|------|
| `README.ja.md` | Presence of Japanese README |
| DeepWiki Mention | Whether a DeepWiki link exists within `README.ja.md` |
| `google*.html` | Presence of Google Search Console verification file |
| `AGENTS.md` / `copilot-instructions.md` | Presence of AI agent instruction files |
| `.github/workflows/*.yml` | Presence of CI/CD workflow |
| `_config.yml` | Presence of Jekyll configuration file |

## Requirements

- Python 3.11 or higher (or Python 3.10 or lower + `pip install tomli`)
- [GitHub CLI](https://cli.github.com/) installed and authenticated with `gh auth login`

## Installation

```bash
git clone https://github.com/cat2151/cat-repo-auditor.git
cd cat-repo-auditor
```

No additional packages required (uses only Python 3.11+ standard library).

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

A summary is displayed in the terminal with Monokai colors.

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
  README.ja.md  [15/20 present / 5/20 absent]
    ✗ some-repo
      https://github.com/your-github-username/some-repo
    ...
```

The JSON file (`repo_analysis.json`) contains detailed information for each repository.

## github_local_checker.py

- A checking tool focused on the local side.
- Uses the same TOML configuration.
- When run normally, it performs a dry-run, checking local repositories and printing the results.
- When run with `--pull`, it pulls all pullable repositories.
- Its purpose is for users with many small experimental repositories to easily manage them by pulling them all locally.

# check_local_workflows.py

- A checking tool focused on the local side.
- Uses the same TOML configuration.
- Checks hashes.
- Its purpose is for users with many small experimental repositories to easily manage them.

# sync_workflows.py

- Its purpose is to synchronize workflow files across local repositories to match the majority.
- A confirmation prompt (y/[N]) is displayed before the final commit push.