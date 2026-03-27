# cat-repo-auditor

An app that lists and visualizes GitHub repository remote/local status and automates parts of maintenance for efficiency. TUI. Rust. For Windows.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Current State

A Rust TUI version has been generated. This is for my personal use.

### Install

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### Run

```
catrepo
```

Print the commit hash embedded at build time:

```
catrepo hash
```

### How to Use

- Since this is for personal use, it's not intended for others. The following are also my personal notes.

- Motivation for creation
  - My hobby OSS repositories have increased. Maintenance is taking up cognitive load and increasing effort.
  - Hence, TUI. I'll create my own TUI to make maintenance easier.
  - Small-scale TUIs can be easily created with Claude's free chat version, so I'll go with that.

- Usage
  - Config
    - Upon first launch, `config.html` will be generated in the local config directory, and its full path will be displayed.
    - Use that as a hint to edit `config.html` yourself.
    - It won't work without editing.
  - Help
    - After launching, press the `?` key to display help.

- PoC
  - The overall usage for this repository is a Proof of Concept.
  - This is to demonstrate that small-scale TUIs like this can be created with Claude's free chat version.
  - So, this is to convey the idea that everyone should try making one for themselves!

## The following is old. This is a separate tool, moved to `python/`. It will be rewritten later.

## Overview

Fetches the 20 most recent repositories of a user authenticated with the `gh` command (GitHub CLI), and automatically checks the following items for each repository. The results are output to a JSON file, and a colored summary is displayed in the terminal.

### Check Items

| Item | Description |
|------|------|
| `README.ja.md` | Existence of Japanese README |
| DeepWiki Link | Whether there is a link to DeepWiki within `README.ja.md` |
| `google*.html` | Existence of verification file for Google Search Console |
| `AGENTS.md` / `copilot-instructions.md` | Existence of instruction files for AI agents |
| `.github/workflows/*.yml` | Existence of CI/CD workflow |
| `_config.yml` | Existence of Jekyll configuration file |

## Prerequisites

- Python 3.11 or higher (or Python 3.10 or lower + `pip install tomli`)
- [GitHub CLI](https://cli.github.com/) must be installed and authenticated with `gh auth login`

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

A summary will be displayed in Monokai colors in the terminal.

```
=== GitHub Repository Analysis CLI ===
Execution Date/Time: 2026-02-23 12:00:00
Target User: your-github-username
Authentication: Obtained via gh auth token

[1/3] Fetching repositories for your-github-username...
      20 repositories fetched

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

# github_local_checker.py

- A check tool focused on the local side.
- Uses the same TOML.
- When run normally, it performs a dry-run check of local repositories and prints the results.
- When run with `--pull`, it pulls all pullable items.
- Its purpose is for users with many small experimental repositories to easily manage them by pulling them all locally.

# check_local_workflows.py

- A check tool focused on the local side.
- Uses the same TOML.
- Performs hash checks.
- Its purpose is for users with many small experimental repositories to easily manage them.

# sync_workflows.py

- Its purpose is to synchronize workflow files across local repositories to match the majority.
- A confirmation prompt (y/[N]) will be displayed before the final commit and push.
