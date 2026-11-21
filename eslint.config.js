'use strict';

const eslint = require('@eslint/js');
const eslintConfigPrettier = require('eslint-config-prettier');
const noUnsanitized = require('eslint-plugin-no-unsanitized');
const globals = require('globals');

/** @type {import('eslint').Linter.Config[]} */
module.exports = [
    {
        ignores: ['node_modules/**'],
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
            'crates/clip-refiner/src/ui/settings/regex_preview.js',
        ],
        languageOptions: {
            globals: {
                module: 'readonly',
            },
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
