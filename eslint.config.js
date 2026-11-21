import eslint from '@eslint/js';
import eslintConfigPrettier from 'eslint-config-prettier';
import noUnsanitized from 'eslint-plugin-no-unsanitized';
import globals from 'globals';

/** @type {import('eslint').Linter.Config[]} */
export default [
    {
        ignores: [
            'node_modules/**',
            'target/**',
            'e2e/**',
            'crates/clip-refiner/src/ui/**/*.bundle.js',
        ],
    },
    eslint.configs.recommended,
    eslintConfigPrettier,
    {
        files: ['eslint.config.js', 'scripts/**/*.mjs'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'module',
            globals: {
                ...globals.node,
            },
        },
    },
    {
        files: ['crates/clip-refiner/src/ui/**/*.js'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'module',
            globals: {
                ...globals.browser,
            },
        },
        plugins: {
            'no-unsanitized': noUnsanitized,
        },
        rules: {
            'no-console': 'off',
            'no-var': 'off',
            'no-unsanitized/method': [
                'error',
                {
                    escape: {
                        methods: [
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
                        ],
                    },
                },
            ],
            'no-unsanitized/property': [
                'error',
                {
                    escape: {
                        methods: [
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
                        ],
                    },
                },
            ],
        },
    },
    {
        files: ['crates/clip-refiner/tests/**/*.js'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'module',
            globals: {
                ...globals.node,
            },
        },
    },
];
