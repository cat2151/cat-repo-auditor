# cat-repo-auditor

A TUI application for Windows, written in Rust, that lists and visualizes the remote/local status of GitHub repositories, automating parts of their maintenance for improved efficiency.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Current State

Generated a Rust version of the TUI. This is for personal use.

### Installation

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### Running

```
catrepo
```

Displays the commit hash embedded during build:

```
catrepo hash
```

### Usage

- This is for personal use and not intended for others. The following are also my personal notes.

- Motivation for Creation
  - My personal open-source repositories have increased. Maintaining them takes cognitive load and effort.
  - Hence, a TUI. I'll create my own TUI to simplify maintenance.
  - Small-scale TUIs can be easily made with the free version of Claude's chat, so I'll use that approach.

- Usage
  - Config
    - Upon first launch, `config.toml` will be generated in the local config directory, and its full path will be displayed.
      - Use that as a hint to manually edit `config.toml`.
      - It will not function without editing.
  - Help
    - Once launched, press the `?` key to display help.

- Proof of Concept (PoC)
  - The overall purpose of this repository is as a PoC.
  - This is to demonstrate that such a small-scale TUI can be created using the free version of Claude's chat.
  - Therefore, it's meant to encourage others to create their own tools!

## The following is old content. It's a separate project, moved to `python/`. I plan to rewrite it later.

## Overview

Retrieves the 20 most recent repositories of a user authenticated with the `gh` command (GitHub CLI), and automatically checks the following items for each repository. The results are output to a JSON file, with a color-coded summary displayed in the terminal.

### Check Items

| Item | Description |
|------|------|
| `README.ja.md` | Presence of Japanese README |
| DeepWiki link | Presence of a DeepWiki link in `README.ja.md` |
| `google*.html` | Presence of Google Search Console verification file |
| `AGENTS.md` / `copilot-instructions.md` | Presence of AI agent instruction file |
| `.github/workflows/*.yml` | Presence of CI/CD workflow |
| `_config.yml` | Presence of Jekyll configuration file |

## Requirements

- Python 3.11 or higher (or Python 3.10 or lower + `pip install tomli`)
- [GitHub CLI](https://cli.github.com/) must be installed and authenticated via `gh auth login`

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

A summary is displayed in Monokai colors in the terminal.

```
=== GitHub Repository Analysis CLI ===
Execution Time: 2026-02-23 12:00:00
Target User: your-github-username
Authentication: Obtained via gh auth token

[1/3] Retrieving repositories for your-github-username...
      20 items retrieved

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

# github_local_checker.py

- A checking tool focused on the local side.
- Uses the same TOML configuration.
- When run normally, it performs a dry-run, checking local repositories and printing the results.
- When run with `--pull`, it pulls all pullable repositories.
- Intended for users with many small experimental repositories to easily keep track of them by pulling them all locally.

# check_local_workflows.py

- A checking tool focused on the local side.
- Uses the same TOML configuration.
- Performs hash checks.
- Intended for users with many small experimental repositories to easily keep track of them.

# sync_workflows.py

- Intended for synchronizing workflow files among local repositories to match the majority.
- A confirmation prompt (y/[N]) is displayed before the final commit and push.