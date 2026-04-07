# cat-repo-auditor

A TUI for listing and visualizing the remote/local status of GitHub repositories, automating part of the maintenance to improve efficiency. Written in Rust.

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## Status

Currently dogfooding.

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

### Display the commit hash embedded during build:

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

- This tool is primarily for personal use and not intended for general public consumption. The following are also personal notes.

- Motivation for creation
  - My personal open-source repositories have grown. Maintenance has become increasingly burdensome due to cognitive load.
  - Hence, a TUI. I'm building my own TUI to simplify maintenance.
  - Small-scale TUIs can be easily created with Claude's free chat version, so I'm going with that approach.

- How to Use
  - config
    - Upon first launch, `config.toml` will be generated in the local config directory, and its full path will be displayed.
      - Use this as a hint to manually edit `config.toml`.
      - It will not function without editing.
  - help
    - Once launched, press the `?` key to display help.

- PoC
  - Regarding the overall purpose of this repository, it's a Proof of Concept (PoC).
  - It serves to demonstrate that small-scale TUIs like this can be created with Claude's free chat version.
  - Therefore, it aims to encourage others to build their own for personal use!