# 開発への参加

ブランチ・コミットの規約、コーディング規約、ドキュメントの同期ルールをまとめる。

## Git 運用

### ブランチ

- 作業ブランチは最新の `origin/develop` から切り、Pull Request の向き先も `develop` にする。`main` へはリリースのときだけ取り込む
- 1 ブランチ 1 目的にする。無関係な修正は別ブランチに分ける
- ブランチ名は `<prefix>/<kebab-case>`。後半は内容が分かる英小文字・数字・ハイフンで書く (例: `fix/ipc-line-limit`, `refactor/split-large-files`)
- prefix は次から選ぶ。新機能と既存機能の変更はどちらも `feature` を使う (`feat` / `update` は使わない)

| prefix | 用途 | コミットの Type |
| :--- | :--- | :--- |
| `feature` | 新機能・既存機能の変更 | `Feat` / `Update` |
| `fix` | 不具合修正 | `Fix` |
| `refactor` | 挙動を変えない内部の整理 | `Refactor` |
| `chore` | ビルド・依存・設定・開発環境 | `Chore` |
| `docs` | ドキュメント | `Docs` |
| `test` | テストの追加・修正 | `Test` |
| `security` | 脆弱性・防御の強化 | `Security` |
| `release` | `release/v{version}` の形式 | `Release` |

### worktree

`git worktree` で作ったブランチは `<ユーザー名>/<作業名>` のような自動の名前が付く。そのまま使わず、最初のコミットの前に規約の名前へ改める。

```powershell
git fetch origin develop

# 新しくブランチを切る (develop を checkout しない)
git switch -c <prefix>/<kebab-case> origin/develop

# 自動命名のブランチを使う (origin/develop より進んでいないことを確認する)
git branch -m <prefix>/<kebab-case>
```

- `develop` は別の worktree が checkout していることがあるため、worktree 内で `develop` を切り替えて `git pull` しない。常に `git fetch` して `origin/develop` を起点にする
- 起点が古いときは、未 push に限り `git rebase origin/develop` で取り込む

### コミット

- コミットメッセージは `<Type>: <日本語の要約>`。Type は上表の `Feat` / `Update` / `Fix` / `Refactor` / `Test` / `Docs` / `Chore` / `Security` (リリースは `Release: v{version}`)
- 要約は体言止めで、不具合は症状と修正を書く (例: `Fix: UTM 除去で URL より後ろのテキストが消える問題を修正`)。句点は付けない
- 1 コミット 1 目的にする。コードを変えたら、対応するテスト・ドキュメント・再生成した `*.bundle.js` と [ドキュメント同期](#ドキュメント同期) の対象を同じコミットに含める
- 利用者に影響する変更は [CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` に同じコミットで追記する。書き方は CHANGELOG.md 冒頭の規約に従う
- [Git フック](testing.md#git-フック) を有効にしておく。`--no-verify` や `CLIP_REFINER_SKIP_HOOK` でのスキップは、原因が分かっていて Pull Request に理由を書けるときだけにする

### Pull Request

- 作成前に [品質チェック](testing.md#品質チェック) を通す。通せなかった項目は本文に書く
- タイトルはコミットメッセージと同じ `<Type>: <日本語の要約>` にする。マージコミットの件名になる
- 本文には次を書く

| 項目 | 内容 |
| :--- | :--- |
| 概要 | 何を変えたか。不具合修正なら症状と原因 |
| 確認 | 実行した品質チェックと手動確認の内容 |
| 利用者への影響 | CHANGELOG へ追記した内容。無いときは「なし」 |

- マージはマージコミットで行い、squash はしない。ブランチの履歴をそのまま残す

## コーディング規約

規約は `.claude/rules/` に置く (Claude Code では対象ファイルを扱うときに自動で読み込まれる)。

| ファイル | 内容 |
| :--- | :--- |
| `comments.md` | Rust のコメントの文体 (常体・句点・半角括弧)、セクション区切り、`///` の構成、`// SAFETY:`、テストの記述 |
| `imports.md` | Rust の `use` 文のグループ順 (`std` → 外部クレート → `self` / `super` / `crate` / 子モジュール)・空行・グループ内の並び |
| `javascript.md` | WebView UI の JavaScript (バンドルの再生成・`innerHTML` のサニタイズ・純粋ロジックの分離・コメント)、E2E の TypeScript |
| `docs.md` | ドキュメント (`CLAUDE.md`・`.claude/rules/` を含む) の文体、`docs/user/` の制約、CHANGELOG の書き方 |

定型作業は `.claude/skills/` のスキルにまとめている。`check` は `check.ps1` を実行して失敗を直し、`changelog` は利用者に影響する変更を `CHANGELOG.md` の `[Unreleased]` に追記する。

JavaScript の整形・静的検査は ESLint / Prettier の設定に従う (`e2e/` は対象外。[JavaScript リンター](testing.md#javascript-リンター-eslint--prettier))。

`.claude/settings.json` は共有の権限設定で、`.env` の読み書きと `*.bundle.js`・`keys/` の直接編集を拒否し、`git push` は確認を挟む。UI の JavaScript を編集すると PostToolUse フックが `npm run build:ui` を実行する。読み取り系の Git コマンドと `git fetch` は許可する。個人用の上書きは `.claude/settings.local.json` に置く (Git 管理外)。

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
| `docs/user/introduction.md` | 利用者 | 同梱ドキュメントの表紙 (概要・はじめて使う手順・ページ一覧) |
| `docs/dev/` | 開発者 | 環境構築・テスト・アーキテクチャ・リリース・開発規約 |
| `crates/clip-refiner-core/README.md` | ライブラリ利用者 | `clip-refiner-core` の feature・公開 API・使用例 |
| `CHANGELOG.md` | 利用者・開発者 | バージョンごとの変更 (`release.ps1` が `[Unreleased]` 節を読むため移動しない) |

`docs/user/` は mdBook の原稿として MSI / ポータブル ZIP に HTML で同梱する ([利用者向けドキュメントの同梱](setup.md#利用者向けドキュメントの同梱))。そのため次を守る。

- 開発者向けの内容 (内部構造・テスト・開発用の環境変数など) は書かず、`docs/dev/` へもリンクしない
- `docs/user/` の外へはリンクしない。変更履歴は `changelog.md` へリンクする
- 「変換」と「加工」の使い分けは、`docs/user/introduction.md` の「用語」に従う
- ページを足したら `docs/user/SUMMARY.md` と、`scripts/common/verify-msi.ps1` の `$MsiRequiredDocPages` にも足す

## ドキュメント同期

設定スキーマ番号・既定値・上限・既定ホットキーは、実装 (`AppConfig` / `consts`)・生成 TOML コメント・[設定リファレンス](../user/configuration.md) / [README.md](../../README.md) がずれないよう `crates/clip-refiner-core/src/config/spec.rs` が検証する。

| 変えたもの | 同じ変更に含めるもの |
| :--- | :--- |
| `CONFIG_VERSION` / 設定キー / 既定値 | `docs/user/configuration.md` の設定項目表、必要なら最小構成例の `version =` |
| `consts` の処理上限 | `docs/user/configuration.md` の [処理の制限](../user/configuration.md#処理の制限)、生成 TOML コメント (`config/serialize/docs.rs`) |
| 既定ホットキー | README のホットキー表、`docs/user/configuration.md` の `[hotkeys]` |
| 利用者向けの挙動 | README / `docs/user/` の該当節、[CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` |
| README のクイックスタート | `docs/user/introduction.md` の「はじめて使う」 |
| UI の JavaScript (`src/ui/`) | 再生成した `*.bundle.js` (`npm run build:ui`。`ui_bundle_flow.rs` が検出) |
| ディレクトリ・ファイルの移動 / 改名 / 削除 | [アーキテクチャ](architecture.md) の構成図と表 (`architecture_doc_flow.rs` が実在しないパスを検出) |

公開 API の破壊的変更は CHANGELOG の **Breaking changes** / **Changed** / **Removed** へ書き、設定スキーマ変更は **Settings** 節にも移行方法を記載する。
