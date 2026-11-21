---
paths:
  - "docs/**/*.md"
  - "README.md"
  - "CHANGELOG.md"
  - "crates/clip-refiner-core/README.md"
---

# ドキュメント規約

どの文書に何を書くかは [docs/dev/contributing.md](../../docs/dev/contributing.md#ドキュメントの配置) に従う。同じ説明は 1 か所にだけ書き、ほかの文書からはリンクで参照する。

## 文体

- 常体 (だ・である調)。です・ます調にしない
- 括弧・コロンは半角を使う
- 表の区切りは `| :--- | :--- |` の形式にそろえる

## `docs/user/` (利用者向け・mdBook 原稿)

MSI / ポータブル ZIP に HTML で同梱するため次を守る。

- 開発者向けの内容 (内部構造・テスト・開発用の環境変数など) は書かず、`docs/dev/` へもリンクしない
- `docs/user/` の外へはリンクしない。変更履歴は `changelog.md` へリンクする
- 「変換」と「加工」の使い分けは `docs/user/introduction.md` の「用語」に従う
- ページを足したら `docs/user/SUMMARY.md` と `scripts/common/verify-msi.ps1` の `$MsiRequiredDocPages` にも足す

## `docs/dev/architecture.md`

- ディレクトリ・命名規則の単位で書き、ファイル単位の責務表を増やさない
- 構成図・表の 1 列目・バッククォートで囲んだパスは `tests/architecture_doc_flow.rs` が実在を検証する。存在しないパスを書かない

## `CHANGELOG.md`

- 利用者に影響する変更だけを `[Unreleased]` に書く。テスト・リファクタリング・ドキュメント整理など利用者から見えない変更は書かない
- 1 項目 1 文・100 字程度まで・体言止め (`〜を追加` / `〜を修正`)。症状と変更後の挙動を書き、原因や内部実装は書かない
- 節は `Breaking changes` / `Settings` / `Added` / `Changed` / `Fixed` / `Security` / `Removed`
- `release.ps1` が `[Unreleased]` 節を読むため、見出し構造を崩さない

## 同期

設定キー・既定値・処理上限・既定ホットキー・`CONFIG_VERSION` を変えたら、`docs/user/configuration.md` と README のホットキー表を同じ変更で更新する (`crates/clip-refiner-core/src/config/spec.rs` が検証する)。
