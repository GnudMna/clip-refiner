# ClipRefiner

クリップボードのテキストを加工する Windows 常駐アプリ。詳細な開発手順・設計は [DEVELOPMENT.md](DEVELOPMENT.md)、設定仕様は [CONFIG.md](CONFIG.md) を参照する。

## 構成

- `crates/clip-refiner-core` — 加工ロジック・設定型 (GUI 非依存)。ライブラリとして公開する API はこちら。feature `clipboard` / `config` を外したテキスト変換のみの構成でもビルド・テストが通ること
- `crates/clip-refiner` — デスクトップアプリ (トレイ常駐・WebView UI・IPC・OS 連携)。core API は re-export しない
- `crates/clip-refiner/src/ui/` — WebView UI の JavaScript。`*.bundle.js` は esbuild 生成物のため直接編集せず、`npm run build:ui` で再生成してソースと同じコミットに含める
- `e2e/` — Playwright + WebView2 による UI E2E
- `scripts/windows/` — 開発者向けエントリポイント (PowerShell 7 で実行)。実処理は `scripts/common/` に置く

第三者クレートの版はルート `Cargo.toml` の `[workspace.dependencies]` に置き、メンバ側は `workspace = true` で参照する。

## コマンド

```powershell
cargo fmt
cargo test                          # 単体テスト + 統合テスト
cargo test --test monitor_flow      # 統合テストを絞る (tests/*_flow.rs)
cargo test -p clip-refiner-core --no-default-features --all-targets
./scripts/windows/clippy.ps1        # clippy (-D warnings)
./scripts/windows/format.ps1        # rustfmt + Prettier
npm run build:ui                    # UI バンドル再生成
npm run lint                        # ESLint

# 完了前の品質チェック一式 (fmt / clippy / deny / eslint / test / e2e)
./scripts/windows/check.ps1
# GUI セッションが無い場合は WebView E2E をスキップする
$env:CLIP_REFINER_SKIP_E2E = '1'; ./scripts/windows/check.ps1
```

## ドキュメント同期

`cargo test` はコードとドキュメントのずれを検出する。次を変えたら同じ変更で更新する。

- 設定キー・既定値・処理上限・既定ホットキー・`CONFIG_VERSION` → CONFIG.md の設定項目表 / 処理の制限、README のホットキー表 (`crates/clip-refiner-core/src/config/spec.rs` が検証)
- UI の JavaScript → `*.bundle.js` を再生成 (`tests/ui_bundle_flow.rs` が検証)
- 利用者に見える挙動 → README / CONFIG の該当節

ドキュメントは常体 (だ・である調) で書く。

## Git

- 作業ブランチは `develop` から切り、PR の向き先も `develop`
- ブランチ名は `<type>/<kebab-case>` (例: `fix/...`, `refactor/...`, `chore/...`, `security/...`, `docs/...`)
- コミットメッセージは `<Type>: <日本語の要約>` (例: `Fix: DPAPI 平文のゼロ化を修正`)。Type は `Feat` / `Update` / `Fix` / `Refactor` / `Test` / `Docs` / `Chore` / `Security`
- 利用者に影響する変更は [CHANGELOG.md](CHANGELOG.md) の `[Unreleased]` に追記する。節は `Breaking changes` / `Settings` / `Added` / `Changed` / `Fixed` / `Security` / `Removed`

## コーディング規約

Rust ソース (`**/*.rs`) の規約は `.claude/rules/` に置く (対象ファイルを扱うときに自動で読み込まれる)。

- [comments.md](.claude/rules/comments.md) — コメント・ドキュメントコメント・`SAFETY` コメント・テストの記述規約
- [imports.md](.claude/rules/imports.md) — `use` 文のグループ分けと並び順
