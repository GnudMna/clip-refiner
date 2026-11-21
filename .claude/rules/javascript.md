---
paths:
  - "crates/clip-refiner/src/ui/**/*.js"
  - "crates/clip-refiner/tests/**/*.js"
  - "scripts/**/*.mjs"
  - "crates/clip-refiner/src/ui/**/*.d.ts"
  - "e2e/**/*.ts"
---

# JavaScript 規約

WebView UI (`crates/clip-refiner/src/ui/`)・Node.js テスト・ビルドスクリプト・E2E (`e2e/*.ts`) の規約。整形は Prettier (`.prettierrc.json`)、静的検査は ESLint (`eslint.config.js`)、型チェックは `tsc` (UI は `src/ui/tsconfig.json` の `checkJs`、E2E は `e2e/tsconfig.json`) に従う。

## バンドル

- `*.bundle.js` は esbuild の生成物のため直接編集しない。ソースを変えたら再生成し、同じコミットに含める (`tests/flow/ui_bundle.rs` が再生成忘れを検出する)
- Claude Code では `src/ui/` の `.js` を編集するたびに PostToolUse フックが `npm run build:ui` を実行する。フックがエラーを返したらビルドエラーを直す。複数ファイルにまたがる変更の途中で出たエラーは、変更を終えてから再確認すればよい
- 新しいエントリや入力を足すときは `scripts/common/build-ui.mjs` の `bundles` に追加する

## HTML の組み立て

- `innerHTML` へ代入してよいのは `escapeHtml` / `highlightText` と、信頼済み HTML を組み立てる関数の戻り値だけとする
- 組み立て関数を新設して `innerHTML` に渡す場合は、`eslint.config.js` の `no-unsanitized` の `escape.methods` (`method` / `property` の両方) に関数名を追加する
- `escapeHtml` の実装は `html_escape.js` に一本化されている。別の場所で再実装しない
- 外部ページへの遷移・外部 script・インライン評価 (`eval` など) を持ち込まない (WebView は CSP で禁止している)

## 構成

- DOM 非依存の純粋ロジックは `*_logic.js` / `selector-common.js` / `html_escape.js` などに置く。`tests/flow/*_logic.rs` が Node.js から `import` して検証するため、`window` / `document` を参照しない
- 設定ウィンドウの共有ミュータブル状態は `settings/ctx.js` に集約し、循環 import を避ける
- ロジックを足したら対応する `tests/flow/*_logic.rs` / `tests/*_logic.js` にテストを足す。画面操作を伴う変更は `e2e/*.spec.ts` も検討する

## 型

- UI は `checkJs` で JSDoc の型を検証する。`Record<string, unknown>` や `Object` で済ませず、扱う値の形を書く
- Rust と受け渡す JSON (設定スナップショット・`SettingsPatch`・IPC メッセージ・`window` に公開する関数の引数) の型は `src/ui/ipc.d.ts` に置き、JSDoc からは `import('../ipc').SettingsSnapshot` のように参照する。Rust の serde 型を変えたら同じ変更で合わせる (ずれは `tray/settings/webview_contract_tests.rs` が `cargo test` で検出する。メッセージ・応答の種類を足したらそこのサンプルにも足す)
- `strictNullChecks` は有効。シェル HTML に必ずある要素は `dom.js` の `requiredById` / `requiredChild` で取得し、描画で差し替わる要素は `null` を確かめる
- `value` / `checked` を参照する要素は `dom.js` の `inputById` / `selectById` / `buttonById` で取得する
- 設定ウィンドウの描画・イベントでは `ctx.state` を直接使わず、`loadedState()` で読み込み済みのスナップショットを得る (`loadSettings` 前に `ctx.state` を扱う箇所だけ `null` を確かめる)
- 複数モジュールで使う typedef はモジュール直下に置く (IIFE の内側に書くと `import()` で参照できない)

## コメント

- 言語は日本語、文体は常体 (だ・である調) または体言止め。です・ます調にしない
- セクション区切りは Rust と同じ形式 (`// ====` 70 文字) を使う
- 関数には JSDoc (`@param` / `@returns`) を付け、型は JSDoc の型注釈で書く
- 括弧・コロンは半角を使う

## E2E

- `e2e/` は Playwright の TypeScript。型は JSDoc ではなく TypeScript の型注釈で書く
- `page.evaluate` で呼ぶ `window.*` の関数は `src/ui/ipc.d.ts` の宣言で型付けされる。`window as any` にしない
- コメントの文体は「コメント」節に従う

## 完了前の確認

UI のバンドルはフックが再生成するため、手動では次を確認する (フックが無い環境では先に `npm run build:ui` を実行する)。

```powershell
npm run lint
npm run format:check
npm run typecheck
```
