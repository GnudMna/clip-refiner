---
paths:
  - "docs/**/*.md"
  - "README.md"
  - "CHANGELOG.md"
  - "crates/clip-refiner-core/README.md"
  - "CLAUDE.md"
  - ".claude/rules/*.md"
  - ".claude/skills/**/*.md"
---

# ドキュメント規約

どの文書に何を書くかは [docs/dev/contributing.md](../../docs/dev/contributing.md#ドキュメントの配置) に従う。同じ説明は 1 か所にだけ書き、ほかの文書からはリンクで参照する。

## 文体

- 常体 (だ・である調)。です・ます調にしない
- 括弧・コロンは半角を使う
- 表の区切りは `| :--- | :--- |` の形式にそろえ、セルは空白で埋めない (列の幅をそろえない)
- 書式は `npm run lint:md` (markdownlint) で検証する。Prettier は表の列をそろえて規約と合わないため Markdown には使わない

## 文書ごとの規約

- `docs/user/` (利用者向け・mdBook 原稿): MSI / ポータブル ZIP に同梱するため、`docs/dev/` など外部へリンクしない。制約の全体は [ドキュメントの配置](../../docs/dev/contributing.md#ドキュメントの配置) に従う
- `docs/dev/architecture.md`: ディレクトリ・命名規則の単位で書き、ファイル単位の責務表を増やさない。パスの検証は同書の冒頭に従う
- `CHANGELOG.md`: 書き方は冒頭の規約に従う。`release.ps1` が `[Unreleased]` 節を読むため、見出し構造を崩さない

## 同期

設定・既定値・UI の JavaScript・ディレクトリ構成などを変えたら、[ドキュメント同期](../../docs/dev/contributing.md#ドキュメント同期) の表にある更新先を同じ変更で更新する。
