# 開発への参加

ブランチ・コミットの規約、コーディング規約、ドキュメントの同期ルールをまとめる。

## Git 運用

- 作業ブランチは `develop` から切り、Pull Request の向き先も `develop`
- ブランチ名は `<type>/<kebab-case>` (例: `fix/ipc-line-limit`, `refactor/split-large-files`, `docs/...`)
- コミットメッセージは `<Type>: <日本語の要約>`。主な Type は `Feat` / `Update` / `Fix` / `Refactor` / `Test` / `Docs` / `Chore` / `Security` (リリースは `Release: v{version}`)
- 利用者に影響する変更は [CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` に同じ変更で追記する
- Pull Request 前に [品質チェック](testing.md#品質チェック) を通す

## コーディング規約

Rust ソースの規約は `.claude/rules/` に置く (Claude Code では対象ファイルを扱うときに自動で読み込まれる)。

| ファイル | 内容 |
| :------- | :--- |
| `comments.md` | コメントの文体 (常体・句点・半角括弧)、セクション区切り、`///` の構成、`// SAFETY:`、テストの記述 |
| `imports.md` | `use` 文のグループ順 (`std` → `super` / `crate` → 外部クレート)・空行・アルファベット順 |

JavaScript は ESLint / Prettier の設定に従う ([JavaScript リンター](testing.md#javascript-リンター-eslint--prettier))。

## ドキュメントの配置

ドキュメントは読み手ごとに分け、同じ説明は 1 か所にだけ書く。ほかの文書からはリンクで参照する。

| 置き場所 | 読み手 | 書くこと |
| :--- | :--- | :--- |
| `README.md` | 初めて使う人 | 概要・インストール・クイックスタート・既定ホットキー・ドキュメント索引 |
| `docs/user/usage.md` | 利用者 | 操作方法 (トレイ・設定ウィンドウ・セレクター・各機能・更新) |
| `docs/user/configuration.md` | 利用者 | `config.toml` の場所・キー・既定値・処理の制限など設定ファイルの仕様 |
| `docs/user/modes.md` | 利用者 | 変換モードの一覧と使用例 |
| `docs/user/security.md` | 利用者 | 保護の仕組みと、保護の対象外となる範囲 |
| `docs/user/troubleshooting.md` | 利用者 | 症状別の確認事項 |
| `docs/dev/` | 開発者 | 環境構築・テスト・アーキテクチャ・リリース・開発規約 |
| `crates/clip-refiner-core/README.md` | ライブラリ利用者 | `clip-refiner-core` の feature・公開 API・使用例 |
| `CHANGELOG.md` | 利用者・開発者 | バージョンごとの変更 (`release.ps1` が `[Unreleased]` 節を読むため移動しない) |

## ドキュメント同期

設定スキーマ番号・既定値・上限・既定ホットキーは、実装 (`AppConfig` / `consts`)・生成 TOML コメント・[設定リファレンス](../user/configuration.md) / [README.md](../../README.md) がずれないよう `crates/clip-refiner-core/src/config/spec.rs` が検証する。

| 変えたもの | 同じ変更に含めるもの |
| :--- | :--- |
| `CONFIG_VERSION` / 設定キー / 既定値 | `docs/user/configuration.md` の設定項目表、必要なら最小構成例の `version =` |
| `consts` の処理上限 | `docs/user/configuration.md` の [処理の制限](../user/configuration.md#処理の制限)、生成 TOML コメント (`config/serialize/docs.rs`) |
| 既定ホットキー | README のホットキー表、`docs/user/configuration.md` の `[hotkeys]` |
| 利用者向けの挙動 | README / `docs/user/` の該当節、[CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` |
| UI の JavaScript (`src/ui/`) | 再生成した `*.bundle.js` (`npm run build:ui`。`ui_bundle_flow.rs` が検出) |

公開 API の破壊的変更は CHANGELOG の **Breaking changes** / **Changed** / **Removed** へ書き、設定スキーマ変更は **Settings** 節にも移行方法を記載する。
