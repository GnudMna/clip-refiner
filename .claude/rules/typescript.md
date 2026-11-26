---
paths:
  - "crates/clip-refiner/src/ui/**/*.ts"
  - "crates/clip-refiner/tests/ui/**/*.ts"
  - "scripts/**/*.ts"
  - "eslint.config.ts"
  - "e2e/**/*.ts"
---

# TypeScript 規約

WebView UI (`crates/clip-refiner/src/ui/`)・Node.js テスト・ビルドスクリプト・E2E (`e2e/`) の規約。整形は Prettier (`.prettierrc.json`)、静的検査は ESLint (`eslint.config.ts`)、型チェックは `tsc` (共通設定は `tsconfig.base.json`) に従う。

## バンドル

- `*.bundle.js` は esbuild の生成物のため直接編集しない。ソースを変えたら再生成し、同じコミットに含める (`tests/flow/ui_bundle.rs` が再生成忘れを検出する)
- Claude Code では `src/ui/` の `.ts` を編集するたびに PostToolUse フックが `npm run build:ui` を実行する。フックがエラーを返したらビルドエラーを直す。複数ファイルにまたがる変更の途中で出たエラーは、変更を終えてから再確認すればよい
- 新しいエントリや入力を足すときは `scripts/common/build-ui.ts` の `bundles` に追加する

## HTML の組み立て

- `innerHTML` へ代入してよいのは `escapeHtml` / `highlightText` と、信頼済み HTML を組み立てる関数の戻り値だけとする
- 組み立て関数を新設して `innerHTML` に渡す場合は、`eslint.config.ts` の `no-unsanitized` の `escape.methods` の `sanitizerMethods` に関数名を追加する (`method` / `property` の両ルールが共有する)
- `escapeHtml` の実装は `shared/html_escape.ts` に一本化されている。別の場所で再実装しない
- 外部ページへの遷移・外部 script・インライン評価 (`eval` など) を持ち込まない (WebView は CSP で禁止している)

## 構成

- ファイル名は snake_case とする (`src/ui/`・`tests/ui/`・`e2e/`)。`scripts/` は PowerShell スクリプトと合わせて kebab-case とする
- DOM 非依存の純粋ロジックは `*_logic.ts` / `selector/selector_common.ts` / `shared/html_escape.ts` などに置く。`tests/flow/*_logic.rs` が Node.js から `import` して検証するため、`window` / `document` を参照しない
- フォルダは画面ごとに分ける (`selector/`・`settings/`)。両方で使うものは `shared/`、E2E のテスト本体は `e2e/specs/`、その補助は `e2e/support/` に置く
- 設定ウィンドウの共有ミュータブル状態は `settings/ctx.ts` に集約し、循環 import を避ける (knip の cycles が検出する)
- ロジックを足したら対応する `tests/flow/*_logic.rs` / `tests/ui/*_logic.ts` にテストを足す。画面操作を伴う変更は `e2e/specs/*.spec.ts` も検討する
- `src/ui/` のカバレッジはファイルごとに下限がある (`npm run test:ui:coverage`)。DOM や WebView の IPC を扱うファイルを足したら `scripts/common/check-ui-coverage.ts` の `unmeasuredFiles` に加える

## 型

- 型は TypeScript の型注釈で書く (JSDoc に型を書かない)。`Record<string, unknown>` や `any` で済ませず、扱う値の形を書く
- Node.js は型注釈を取り除くだけで実行するため、`enum`・`namespace` などの除去できない構文は使わない (`erasableSyntaxOnly`)。相対 import は `.ts` の拡張子を付け、型だけの import は `import type` で書く
- Rust と受け渡す JSON (設定スナップショット・`SettingsPatch`・IPC メッセージ・`window` に公開する関数の引数) の型は `src/ui/ipc.ts` に置き、`import type` で参照する。Rust の serde 型を変えたら同じ変更で合わせる (ずれは `tray/settings/webview_contract_tests.rs` が `cargo test` で検出する。メッセージ・応答の種類を足したらそこのサンプルにも足す)
- `strictNullChecks` は有効。シェル HTML に必ずある要素は `shared/dom.ts` の `requiredById` / `requiredChild` で取得し、描画で差し替わる要素は `null` を確かめる
- `value` / `checked` を参照する要素は `shared/dom.ts` の `inputById` / `selectById` / `buttonById` で取得する
- 設定ウィンドウの描画・イベントでは `ctx.state` を直接使わず、`loadedState()` で読み込み済みのスナップショットを得る (`loadSettings` 前に `ctx.state` を扱う箇所だけ `null` を確かめる)
- 型の宣言はモジュール直下に置き、複数モジュールで使うものだけ `export` する (IIFE の内側では `export` できない)

## コメント

- 言語は日本語、文体は常体 (だ・である調) または体言止め。です・ます調にしない
- セクション区切りは Rust と同じ形式 (`// ====` 70 文字) を使う
- 関数には JSDoc で要約を付ける。`@param` / `@returns` は、名前と型から分からない補足があるときだけ書く
- 括弧・コロンは半角を使う

## E2E

- `page.evaluate` で呼ぶ `window.*` の関数は `src/ui/ipc.ts` の宣言で型付けされる (`e2e/support/window.d.ts` が読み込む)。`window as any` にしない
- コメントの文体は「コメント」節に従う

## 完了前の確認

UI のバンドルはフックが再生成するため、手動では次を確認する (フックが無い環境では先に `npm run build:ui` を実行する)。

```powershell
npm run lint
npm run format:check
npm run typecheck
npm run knip
npm run test:ui:coverage
```
