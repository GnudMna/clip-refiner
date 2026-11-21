# ClipRefiner

クリップボードのテキストを加工する Windows 常駐アプリ。開発者向けドキュメントは [docs/dev/](docs/dev/) (環境構築・テスト・アーキテクチャ・リリース・開発規約)、利用者向けは [docs/user/](docs/user/) にあり、設定仕様は [docs/user/configuration.md](docs/user/configuration.md) を参照する。どの文書に何を書くかは [docs/dev/contributing.md のドキュメントの配置](docs/dev/contributing.md#ドキュメントの配置) に従う。

## 構成

- `crates/clip-refiner-core` — 加工ロジック・設定型 (GUI 非依存)。ライブラリとして公開する API はこちら。feature `clipboard` / `config` を外したテキスト変換のみの構成でもビルド・テストが通ること
- `crates/clip-refiner` — デスクトップアプリ (トレイ常駐・WebView UI・IPC・OS 連携)。core API は re-export しない
- `crates/clip-refiner/src/ui/` — WebView UI の JavaScript。`*.bundle.js` は esbuild 生成物のため直接編集せず、`npm run build:ui` で再生成してソースと同じコミットに含める
- `e2e/` — Playwright + WebView2 による UI E2E
- `crates/clip-refiner-core/benches/` — 変換の処理時間ベンチ (criterion)。`cargo test --all-targets` でも 1 回ずつ実行され、入力が素通りしていないことを検証する
- `fuzz/` — パーサ系変換の cargo-fuzz ターゲット (nightly が必要なためワークスペース外。`check.ps1` に含めない)
- `scripts/windows/` — 開発者向けエントリポイント (PowerShell 7 で実行)。実処理は `scripts/common/` に置く

第三者クレートの版はルート `Cargo.toml` の `[workspace.dependencies]` に置き、メンバ側は `workspace = true` で参照する。

## コマンド

```powershell
cargo fmt
cargo test --all-targets --features test-helpers  # 単体テスト + 統合テスト (check.ps1 と同じ。--release でも通る)
cargo test --test monitor_flow      # 統合テストを絞る (tests/*_flow.rs)
cargo test -p clip-refiner-core --no-default-features --all-targets
./scripts/windows/clippy.ps1        # clippy (-D warnings)
./scripts/windows/format.ps1        # rustfmt + Prettier
npm run build:ui                    # UI バンドル再生成
npm run lint                        # ESLint
./scripts/windows/fuzz.ps1          # ファジング (nightly + cargo-fuzz。不定期にローカルで実行)
./scripts/windows/bench.ps1         # 処理時間ベンチ (criterion。-Save / -Compare で基準と比較)

# 完了前の品質チェック一式 (fmt / clippy / deny / eslint / test / e2e)
./scripts/windows/check.ps1
# GUI セッションが無い場合は WebView E2E をスキップする
$env:CLIP_REFINER_SKIP_E2E = '1'; ./scripts/windows/check.ps1
```

## ドキュメント同期

`cargo test` はコードとドキュメントのずれを検出する。次を変えたら同じ変更で更新する。

- 設定キー・既定値・処理上限・既定ホットキー・`CONFIG_VERSION` → `docs/user/configuration.md` の設定項目表 / 処理の制限、README のホットキー表 (`crates/clip-refiner-core/src/config/spec.rs` が検証)
- UI の JavaScript → `*.bundle.js` を再生成 (`tests/ui_bundle_flow.rs` が検証)
- ディレクトリ・ファイルの移動 / 改名 / 削除 → `docs/dev/architecture.md` の構成図と表 (`tests/architecture_doc_flow.rs` が検証)。同書はディレクトリ・命名規則の単位で書き、ファイル単位の責務表を増やさない
- 利用者に見える挙動 → README / `docs/user/` の該当節

ドキュメントは常体 (だ・である調) で書く。

## Git

- 作業ブランチは `develop` から切り、PR の向き先も `develop`
- ブランチ名は `<type>/<kebab-case>` (例: `fix/...`, `refactor/...`, `chore/...`, `security/...`, `docs/...`)
- コミットメッセージは `<Type>: <日本語の要約>` (例: `Fix: DPAPI 平文のゼロ化を修正`)。Type は `Feat` / `Update` / `Fix` / `Refactor` / `Test` / `Docs` / `Chore` / `Security`
- 利用者に影響する変更は [CHANGELOG.md](CHANGELOG.md) の `[Unreleased]` に追記する。節は `Breaking changes` / `Settings` / `Added` / `Changed` / `Fixed` / `Security` / `Removed`

## コーディング規約

規約は `.claude/rules/` に置く (対象ファイルを扱うときに自動で読み込まれる)。

- [comments.md](.claude/rules/comments.md) — Rust のコメント・ドキュメントコメント・`SAFETY` コメント・テストの記述規約
- [imports.md](.claude/rules/imports.md) — Rust の `use` 文のグループ分けと並び順
- [javascript.md](.claude/rules/javascript.md) — WebView UI の JavaScript (バンドル・`innerHTML` のサニタイズ・純粋ロジックの分離)
- [docs.md](.claude/rules/docs.md) — ドキュメントの文体・`docs/user/` の制約・CHANGELOG の書き方

`.claude/settings.json` は共有の権限設定を持つ。`.env` の読み書きと `*.bundle.js`・`keys/` の直接編集を拒否し、`git push` は確認を挟む。個人用の上書きは `.claude/settings.local.json` に置く (Git 管理外)。

## 注意点

- `unwrap` / `expect` は clippy で警告になる (テストは除く)。`unsafe` には `// SAFETY:` を付ける
- Git フック (`scripts/git/hooks/`: pre-commit は fmt、pre-push は `check.ps1`) は `./scripts/windows/install-hooks.ps1` で有効化できる。スキップは `CLIP_REFINER_SKIP_HOOK=1` (pre-commit / pre-push)、`CLIP_REFINER_SKIP_E2E=1` (E2E のみ)
- `docs/user/` は MSI / ZIP に同梱する mdBook 原稿のため、`docs/dev/` など外部へリンクしない
