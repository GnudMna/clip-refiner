import eslint from '@eslint/js';
import { defineConfig } from 'eslint/config';
import eslintConfigPrettier from 'eslint-config-prettier';
import noUnsanitized from 'eslint-plugin-no-unsanitized';
import tseslint from 'typescript-eslint';

/**
 * innerHTML へ渡してよい (エスケープ済み HTML を返す) 関数名。
 * no-unsanitized の method / property の両ルールで共有する。
 */
const sanitizerMethods = [
    'escapeHtml',
    'highlightText',
    'buildPipelineInnerHtml',
    'buildSidebarNavInnerHtml',
    'buildItemInnerHtml',
    'buildFavoriteRowsHtml',
    'renderClipRow',
    'resolvePanelInnerHtml',
    'buildCategoryHeaderInnerHtml',
    'buildRegisterRowInnerHtml',
];

const unsanitizedOptions = { escape: { methods: sanitizerMethods } };

export default defineConfig([
    {
        ignores: ['node_modules/**', 'target/**', 'crates/clip-refiner/src/ui/**/*.bundle.js'],
    },
    eslint.configs.recommended,
    tseslint.configs.recommended,
    eslintConfigPrettier,
    {
        files: ['crates/clip-refiner/src/ui/**/*.ts'],
        plugins: {
            'no-unsanitized': noUnsanitized,
        },
        rules: {
            'no-console': 'off',
            'no-unsanitized/method': ['error', unsanitizedOptions],
            'no-unsanitized/property': ['error', unsanitizedOptions],
        },
    },
    {
        // 型情報つきの推奨ルールを全 TypeScript (UI・テスト・scripts・E2E・ESLint 設定) に適用する。
        // await 漏れ (テストが素通りする) や any の素通りを検出する。
        files: ['**/*.ts'],
        extends: [tseslint.configs.recommendedTypeChecked],
        languageOptions: {
            parserOptions: {
                projectService: true,
                tsconfigRootDir: import.meta.dirname,
            },
        },
        rules: {
            '@typescript-eslint/no-floating-promises': [
                'error',
                {
                    // トップレベルの test() は node:test が完了を追跡するため await 不要。
                    // サブテスト (t.test) や assert.rejects の await 漏れは検出する
                    allowForKnownSafeCalls: [
                        { from: 'package', package: 'node:test', name: ['test', 'describe', 'it'] },
                    ],
                },
            ],
            '@typescript-eslint/no-misused-promises': 'error',
        },
    },
    {
        // allowForKnownSafeCalls は TestContext#test (t.test) も除外してしまうため、
        // サブテストの await 漏れはここで構文として検出する
        files: ['crates/clip-refiner/tests/ui/**/*.ts'],
        rules: {
            'no-restricted-syntax': [
                'error',
                {
                    selector:
                        "ExpressionStatement > CallExpression[callee.type='MemberExpression'][callee.property.name=/^(test|describe|it)$/]",
                    message: 'サブテストは await する (t.test の戻り値は Promise)',
                },
            ],
        },
    },
    {
        // tsconfig の lib に dom があるため、型チェックでは window / document を書いても通ってしまう。
        // Node (node:test) で単体テストする純粋ロジックは、ここで DOM・ブラウザ API への依存を禁止する。
        // 型としての HTMLElement などは対象外 (値としての参照だけを禁止する)
        files: [
            'crates/clip-refiner/src/ui/**/*_logic.ts',
            'crates/clip-refiner/src/ui/selector/selector_common.ts',
        ],
        rules: {
            'no-restricted-globals': [
                'error',
                ...[
                    'window',
                    'document',
                    'globalThis',
                    'self',
                    'navigator',
                    'location',
                    'history',
                    'localStorage',
                    'sessionStorage',
                    'requestAnimationFrame',
                    'getComputedStyle',
                    'CSS',
                ].map((name) => ({
                    name,
                    message: `${name} は DOM 依存。このファイルは Node で実行できる純粋ロジックに保つ。DOM・IPC を使う処理は呼び出し側 (画面ごとの .ts / selector_ipc.ts) へ置く`,
                })),
            ],
        },
    },
    {
        // ipc.ts の Window 宣言を迂回するキャストの再発を防ぐ
        files: ['e2e/**/*.ts'],
        rules: {
            'no-restricted-syntax': [
                'error',
                {
                    selector: "TSAsExpression[expression.name='window']",
                    message:
                        'window を as で型変換しない。関数の型は src/ui/ipc.ts の Window 宣言に足す',
                },
            ],
        },
    },
]);
