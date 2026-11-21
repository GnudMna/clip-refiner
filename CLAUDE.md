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
cargo test --test flow monitor::    # 統合テストを絞る (tests/flow/ の 1 モジュール = 1 ファイル)
cargo test -p clip-refiner-core --no-default-features --all-targets
./scripts/windows/clippy.ps1        # clippy (-D warnings)
./scripts/windows/format.ps1        # rustfmt + Prettier + markdownlint の自動修正
npm run build:ui                    # UI バンドル再生成
npm run lint                        # ESLint
npm run lint:md                     # markdownlint (Markdown の表などの書式)
npm run typecheck                   # TypeScript 型チェック (UI の checkJs と E2E)
./scripts/windows/fuzz.ps1          # ファジング (nightly + cargo-fuzz。不定期にローカルで実行)
./scripts/windows/bench.ps1         # 処理時間ベンチ (criterion。-Save / -Compare で基準と比較)

# 完了前の品質チェック一式 (fmt / clippy / deny / eslint / prettier / tsc / markdownlint / test / e2e)
./scripts/windows/check.ps1
# GUI セッションが無い場合は WebView E2E をスキップする
$env:CLIP_REFINER_SKIP_E2E = '1'; ./scripts/windows/check.ps1
```

## ドキュメント同期

設定 (キー・既定値・上限・ホットキー・`CONFIG_VERSION`)・UI の JavaScript・ディレクトリ構成・利用者に見える挙動を変えたら、同じ変更で対応するドキュメントも更新する。更新先は [docs/dev/contributing.md のドキュメント同期](docs/dev/contributing.md#ドキュメント同期) の表に従う。ずれは `cargo test` が検出する。

## Git

ブランチ名 (prefix の一覧)・コミットメッセージ・CHANGELOG の追記・PR の書き方は [docs/dev/contributing.md の Git 運用](docs/dev/contributing.md#git-運用) に従う。ここには作業時に間違えやすい点だけを置く。

- ブランチは `develop` を checkout / pull せず、`git fetch origin develop` のうえ `origin/develop` から切る (`git switch -c <prefix>/<kebab-case> origin/develop`)。グローバルの「develop で `git pull`」より、このリポジトリではこちらを優先する
- worktree の自動命名ブランチは、最初のコミットの前に `git branch -m` で規約の名前へ改める。新機能・既存機能の変更の prefix は `feat` ではなく `feature`
- フックのスキップ (`--no-verify` / `CLIP_REFINER_SKIP_HOOK`) は指示がない限り使わない

## コーディング規約

規約は `.claude/rules/` (`comments.md` / `imports.md` / `javascript.md` / `docs.md`) に置き、対象ファイルを扱うときに自動で読み込まれる。品質チェックと CHANGELOG 追記はスキル `/check`・`/changelog` (`.claude/skills/`) で行う。共有の権限設定とフックは `.claude/settings.json`、個人用の上書きは `.claude/settings.local.json` (Git 管理外) に置く。

## 注意点

- `unwrap` / `expect` は clippy で警告になる (テストは除く)
- Git フック (`scripts/git/hooks/`: pre-commit は fmt、pre-push は E2E 抜きの `check.ps1 -SkipE2E`) は `./scripts/windows/install-hooks.ps1` で有効化できる。E2E は pre-push に含まれないため、`/check` の実行時とリリース前に必ず通す。スキップは `CLIP_REFINER_SKIP_HOOK=1` (pre-commit / pre-push)、`CLIP_REFINER_SKIP_E2E=1` (`check.ps1` の E2E のみ)
