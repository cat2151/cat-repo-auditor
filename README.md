# cat-repo-auditor

A TUI written in Rust that lists, visualizes, and automates part of the maintenance of GitHub repositories to streamline the process.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Status

Dogfooding in progress.

### install

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### Run

```
catrepo
```

### update

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

### Check whether an update is available:

```
catrepo check
```

### Show CLI help:

```
catrepo help
catrepo --help
```

### Usage

- This is for personal use and not intended for others. The following are also personal notes.

- Motivation for Creation
  - My personal OSS repositories have increased. Maintaining them takes cognitive load and effort.
  - Hence, a TUI. I'm building my own TUI to simplify maintenance.
  - For small-scale TUIs, it's easy to create them using Claude's free chat version, so I'll go with that.

- How to Use
  - config
    - Upon first launch, `config.toml` will be generated in your local config directory, and its full path will be displayed.
      - Use that as a hint to manually edit `config.toml`.
      - It will not work without editing.
  - help
    - Once launched, press the `?` key to display help.

- PoC
  - Regarding the overall usage of this repository, it's a PoC (Proof of Concept).
  - This is to demonstrate that small-scale TUIs like this can be created using Claude's free chat version.
  - So, it's meant to encourage everyone to create one for themselves!

## The following is old. It's a separate entity. Moved to python/. Will rewrite later.

## Overview

Retrieves the latest 20 repositories of a user authenticated with the `gh` command (GitHub CLI), and automatically checks the following items for each repository. The results are output to a JSON file, and a colorized summary is displayed in the terminal.

### Check Items

| Item | Description |
|------|------|
| `README.ja.md` | Presence of Japanese README |
| DeepWiki Entry | Link to DeepWiki in `README.ja.md` |
| `google*.html` | Presence of verification file for Google Search Console |
| `AGENTS.md` / `copilot-instructions.md` | Presence of instruction files for AI agents |
| `.github/workflows/*.yml` | Presence of CI/CD workflows |
| `_config.yml` | Presence of Jekyll configuration file |

## Requirements

- Python 3.11+ (or Python 3.10- + `pip install tomli`)
- [GitHub CLI](https://cli.github.com/) installed and authenticated with `gh auth login`

## Installation

```bash
git clone https://github.com/cat2151/cat-repo-auditor.git
cd cat-repo-auditor
```

No additional packages required (uses only Python 3.11+ standard library).

For Python 3.10 or earlier:

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

## github_local_checker.py

- A check tool centered on the local side
- Uses the same TOML
- When run normally, it performs a dry-run check of local repositories and prints the results.
- When run with `--pull`, it pulls all pullable repositories.
- Its purpose is to help users with many small experimental repositories manage them easily by bulk-pulling them locally.

# check_local_workflows.py

- A check tool centered on the local side
- Uses the same TOML
- Performs hash checks
- Its purpose is to help users with many small experimental repositories manage them easily.

# sync_workflows.py

- Its purpose is to synchronize workflow files across local repositories to match the majority.
- Before the final commit and push, a confirmation prompt (y/[N]) will be displayed.
