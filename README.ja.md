# cat-repo-auditor

GitHubリポジトリのremote/local状況をlistし、可視化し、メンテの一部を自動化して効率化するTUI。Rustで書かれています。

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## 状況

ドッグフーディング中です。

### install

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### 実行

```
catrepo
```

### update

```
catrepo update
```

もし失敗したら以下を実行（installコマンドと同じ）：

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### ビルド時に埋め込まれた commit hash を表示する:

```
catrepo hash
```

### update があるか確認する:

```
catrepo check
```

### CLIヘルプを表示する:

```
catrepo help
catrepo --help
```

### 使い方

- 自分用なので他人が使うことを想定していません。以下も自分用のメモです。

- 作ったモチベ
  - 趣味OSSのリポジトリが増えてきました。整備に認知負荷をとられて手間が増えています。
  - そこでTUI。TUIを自作して整備を楽にします。
  - TUIは小規模ならClaude無料版chatで楽に作れるので、それでいきます。

- 使い方
  - config
    - 初回起動時に、config.toml が local config dirに生成され、そのfullpathが表示されます。
      - それをヒントにして、config.tomlを自力で編集してください。
      - 編集しないと動きません。
  - help
    - 起動したら、`?` キーを押すとhelpが出ます。

- PoC
  - リポジトリ全体の使い方としては、PoC。
  - これくらいの小規模なTUIなら、Claude無料版chatで作れます、を実証する用です。
  - なので、みんなも自分用に作ろう！というのを伝える用です。
