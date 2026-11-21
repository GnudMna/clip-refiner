---
paths:
  - "crates/clip-refiner/src/ui/**/*.js"
  - "crates/clip-refiner/tests/**/*.js"
  - "scripts/**/*.mjs"
  - "e2e/**/*.ts"
---

# JavaScript 規約

WebView UI (`crates/clip-refiner/src/ui/`) とビルドスクリプトの規約。整形は Prettier (`.prettierrc.json`)、静的検査は ESLint (`eslint.config.js`) に従う。

## バンドル

- `*.bundle.js` は esbuild の生成物のため直接編集しない。ソースを変えたら `npm run build:ui` で再生成し、同じコミットに含める (`tests/ui_bundle_flow.rs` が再生成忘れを検出する)
- 新しいエントリや入力を足すときは `scripts/common/build-ui.mjs` の `bundles` に追加する

## HTML の組み立て

- `innerHTML` へ代入してよいのは `escapeHtml` / `highlightText` と、信頼済み HTML を組み立てる関数の戻り値だけとする
- 組み立て関数を新設して `innerHTML` に渡す場合は、`eslint.config.js` の `no-unsanitized` の `escape.methods` (`method` / `property` の両方) に関数名を追加する
- `escapeHtml` の実装は `html_escape.js` に一本化されている。別の場所で再実装しない
- 外部ページへの遷移・外部 script・インライン評価 (`eval` など) を持ち込まない (WebView は CSP で禁止している)

## 構成

- DOM 非依存の純粋ロジックは `*_logic.js` / `selector-common.js` / `html_escape.js` などに置く。`tests/*_logic.rs` が Node.js から `import` して検証するため、`window` / `document` を参照しない
- 設定ウィンドウの共有ミュータブル状態は `settings/ctx.js` に集約し、循環 import を避ける
- ロジックを足したら対応する `tests/*_logic.rs` / `*_logic.js` にテストを足す。画面操作を伴う変更は `e2e/*.spec.ts` も検討する

## コメント

- 言語は日本語、文体は常体 (だ・である調) または体言止め。です・ます調にしない
- セクション区切りは Rust と同じ形式 (`// ====` 70 文字) を使う
- 関数には JSDoc (`@param` / `@returns`) を付け、型は JSDoc の型注釈で書く
- 括弧・コロンは半角を使う

## 完了前の確認

```powershell
npm run build:ui
npm run lint
npm run format:check
```
