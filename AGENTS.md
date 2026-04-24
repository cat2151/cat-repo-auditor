#
- ソース机上チェックだけで実測をしない憶測の実装は禁止する。
    - 実測とは、appdata config dirs のlogを調査し、関数を実行して再現チェックを行う、など。
        - データ不足なら報告すること。
    - cargo install hash 判定を修正するときの禁止事項:
        - `.crates2.json` を常に正として扱う実装は禁止する。
        - 最新 checkout directory を常に正として扱う実装は禁止する。
        - source conflict がある状態で old/ok を断定し、auto-update loop に入れる実装は禁止する。
        - `.cargo/git/checkouts` をアプリが勝手に削除する実装は禁止する。必要なら overlay で対象 directory と根拠を表示し、人間に削除を促す。
    - cargo install hash 判定の方針:
        - 実行バイナリが `hash` / `check` で embedded hash を返す場合は、それを最優先の installed hash とする。
        - `.crates2.json` と checkout HEAD が一致する場合は高信頼で扱う。
        - `.crates2.json` と checkout HEAD が食い違い、実行バイナリでも確認できない場合は ambiguous/warning として扱い、old/ok を断定しない。
        - `ym2151-log-play-server` のように binary self-report、metadata、remote が一致し、最新 checkout だけ外れている場合は cargo check 自体は OK 扱いにしつつ、異常 checkout directory の overlay 警告を出す。
#
- issue-notes/ は参照はするが更新はしないこと。更新するのは人間だけ。更新するかわりにプルリクエストのコメントに日本語で書くこと。
- プルリクエストは日本語で書くこと。
- cat2151のライブラリはrevision指定を禁止する。毎日重要なバグ修正があるため、revisionを固定してしまうとバグ修正を取り込めないため。
- README.md編集禁止。README.ja.mdがSSoTであり、README.mdはREADME.ja.mdから生成されるので。
