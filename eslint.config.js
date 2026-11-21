'use strict';

const eslint = require('@eslint/js');
const eslintConfigPrettier = require('eslint-config-prettier');
const noUnsanitized = require('eslint-plugin-no-unsanitized');
const globals = require('globals');

/** @type {import('eslint').Linter.Config[]} */
module.exports = [
    {
        ignores: ['node_modules/**', 'e2e/**'],
    },
    eslint.configs.recommended,
    eslintConfigPrettier,
    {
        files: ['eslint.config.js'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'commonjs',
            globals: {
                ...globals.node,
            },
        },
    },
    {
        files: ['crates/clip-refiner/src/ui/**/*.js'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'script',
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
                        ],
                    },
                },
            ],
        },
    },
    {
        // Rust 側で 1 つの IIFE に連結するため、ファイル間の共有バインディングは ESLint 単体では解決できない
        files: [
            'crates/clip-refiner/src/ui/settings/shared.js',
            'crates/clip-refiner/src/ui/settings/html_builders.js',
            'crates/clip-refiner/src/ui/settings/ipc_apply.js',
            'crates/clip-refiner/src/ui/settings/confirm_dialog.js',
            'crates/clip-refiner/src/ui/settings/regex_feedback.js',
            'crates/clip-refiner/src/ui/settings/monitor_ui.js',
            'crates/clip-refiner/src/ui/settings/panels.js',
            'crates/clip-refiner/src/ui/settings/events.js',
            'crates/clip-refiner/src/ui/settings/main.js',
        ],
        rules: {
            'no-undef': 'off',
            'no-unused-vars': 'off',
            'no-redeclare': 'off',
        },
    },
    {
        files: [
            'crates/clip-refiner/src/ui/settings/settings_patch.js',
            'crates/clip-refiner/src/ui/settings/shared_logic.js',
            'crates/clip-refiner/src/ui/settings/panels_logic.js',
            'crates/clip-refiner/src/ui/settings/regex_preview.js',
            'crates/clip-refiner/src/ui/selector-common.js',
            'crates/clip-refiner/src/ui/quick_selector_logic.js',
            'crates/clip-refiner/src/ui/clip_selector_logic.js',
        ],
        languageOptions: {
            globals: {
                module: 'readonly',
            },
        },
    },
    {
        // セレクター UI は HTML で順に読み込むため、ファイル間の共有バインディングは ESLint 単体では解決できない
        files: [
            'crates/clip-refiner/src/ui/quick_selector.js',
            'crates/clip-refiner/src/ui/clip_selector.js',
        ],
        rules: {
            'no-undef': 'off',
            'no-unused-vars': 'off',
        },
    },
    {
        files: ['crates/clip-refiner/tests/**/*.js'],
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'commonjs',
            globals: {
                ...globals.node,
            },
        },
    },
];
