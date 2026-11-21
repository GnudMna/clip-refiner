// ======================================================================
// 正規表現タブ UI (プリセット / ステップ)
// ======================================================================

import { ctx } from './ctx.js';
import { escapeHtml, optionCheckbox, wrappedTable } from './html_builders.js';
import { maxRegexPresets, maxRegexStepsPerPreset } from './shared.js';
import RegexPreview from './regex_preview.js';

/**
 * 正規表現設定の既定形を保証する
 *
 * @param {import('./regex_preview').RegexSettings|null|undefined} regex
 * @returns {import('./regex_preview').RegexSettings}
 */
export function normalizeRegexState(regex) {
    const copy = RegexPreview.copyRegexSettings(
        regex && typeof regex === 'object' ? regex : { activePresetId: 'default', presets: [] }
    );
    if (!copy.presets.length) {
        copy.presets = [
            {
                id: 'default',
                name: 'デフォルト',
                steps: [emptyStep()],
            },
        ];
        copy.activePresetId = 'default';
    }
    if (!copy.presets.some((preset) => preset.id === copy.activePresetId)) {
        copy.activePresetId = copy.presets[0].id;
    }
    return copy;
}

/** @returns {import('./regex_preview').RegexStep} */
export function emptyStep() {
    return {
        pattern: '',
        replacement: '',
        caseInsensitive: false,
        multiline: false,
    };
}

/**
 * 編集中プリセット ID を返す
 *
 * @returns {string}
 */
export function selectedRegexPresetId() {
    const regex = normalizeRegexState(ctx.state?.regex);
    if (
        ctx.selectedRegexPresetId &&
        regex.presets.some((preset) => preset.id === ctx.selectedRegexPresetId)
    ) {
        return ctx.selectedRegexPresetId;
    }
    return regex.activePresetId;
}

/**
 * DOM と state から正規表現設定を組み立てる
 *
 * プレビュー用に `activePresetId` を編集中プリセットへ合わせる
 *
 * @returns {import('./regex_preview').RegexSettings}
 */
export function readRegexFromDom() {
    const regex = normalizeRegexState(ctx.state?.regex);
    const selectedId = selectedRegexPresetId();
    const presetIndex = regex.presets.findIndex((preset) => preset.id === selectedId);
    if (presetIndex < 0) {
        return regex;
    }

    const nameInput = document.getElementById('regex-preset-name');
    if (nameInput) {
        regex.presets[presetIndex].name = nameInput.value;
    }

    const stepCount = regex.presets[presetIndex].steps.length;
    const steps = [];
    for (let i = 0; i < stepCount; i += 1) {
        steps.push({
            pattern: document.getElementById('regex-step-pattern-' + i)?.value ?? '',
            replacement: document.getElementById('regex-step-replacement-' + i)?.value ?? '',
            caseInsensitive: !!document.getElementById('regex-step-ci-' + i)?.checked,
            multiline: !!document.getElementById('regex-step-ml-' + i)?.checked,
        });
    }
    if (steps.length === 0) {
        steps.push(emptyStep());
    }
    regex.presets[presetIndex].steps = steps;
    regex.activePresetId = selectedId;
    return regex;
}

/**
 * 保存用の正規表現設定を組み立てる (有効プリセット ID を維持)
 *
 * @returns {import('./regex_preview').RegexSettings}
 */
export function readRegexForApply() {
    const activeId = normalizeRegexState(ctx.state?.regex).activePresetId;
    const settings = readRegexFromDom();
    if (settings.presets.some((preset) => preset.id === activeId)) {
        settings.activePresetId = activeId;
    }
    return settings;
}

/**
 * プリセット一覧とステップ編集の HTML を組み立てる
 *
 * @returns {string}
 */
export function buildRegexEditorHtml() {
    const regex = normalizeRegexState(ctx.state?.regex);
    const selectedId = selectedRegexPresetId();
    const selected = regex.presets.find((preset) => preset.id === selectedId) || regex.presets[0];

    const presetRows = regex.presets
        .map((preset, index) => {
            const isActive = preset.id === regex.activePresetId;
            const isSelected = preset.id === selectedId;
            return (
                '<tr class="' +
                (isSelected ? 'regex-preset-row--selected' : '') +
                '">' +
                '<td class="regex-preset-active">' +
                '<input type="radio" name="regex-active-preset" class="regex-active-radio" data-id="' +
                escapeHtml(preset.id) +
                '"' +
                (isActive ? ' checked' : '') +
                ' title="有効にする" aria-label="有効にする">' +
                '</td>' +
                '<td>' +
                '<button type="button" class="regex-preset-select" data-id="' +
                escapeHtml(preset.id) +
                '">' +
                escapeHtml(preset.name || '(無題)') +
                '</button>' +
                '</td>' +
                '<td class="list-actions">' +
                '<button type="button" class="icon-btn regex-preset-up" data-index="' +
                index +
                '" title="上へ">↑</button>' +
                '<button type="button" class="icon-btn regex-preset-down" data-index="' +
                index +
                '" title="下へ">↓</button>' +
                '<button type="button" class="icon-btn regex-preset-remove" data-index="' +
                index +
                '" title="削除">×</button>' +
                '</td></tr>'
            );
        })
        .join('');

    const stepRows = (selected.steps || [])
        .map((step, index) => {
            return (
                '<div class="regex-step" data-index="' +
                index +
                '">' +
                '<div class="regex-step-header">' +
                '<span class="regex-step-index">' +
                (index + 1) +
                '</span>' +
                '<span class="regex-step-label">ステップ ' +
                (index + 1) +
                '</span>' +
                '<span class="list-actions">' +
                '<button type="button" class="icon-btn regex-step-up" data-index="' +
                index +
                '" title="上へ">↑</button>' +
                '<button type="button" class="icon-btn regex-step-down" data-index="' +
                index +
                '" title="下へ">↓</button>' +
                '<button type="button" class="icon-btn regex-step-remove" data-index="' +
                index +
                '" title="削除">×</button>' +
                '</span></div>' +
                '<div class="field"><label for="regex-step-pattern-' +
                index +
                '">パターン</label>' +
                '<textarea id="regex-step-pattern-' +
                index +
                '" class="regex-step-pattern">' +
                escapeHtml(step.pattern || '') +
                '</textarea></div>' +
                '<div class="field"><label for="regex-step-replacement-' +
                index +
                '">置換文字列</label>' +
                '<textarea id="regex-step-replacement-' +
                index +
                '" class="regex-step-replacement">' +
                escapeHtml(step.replacement || '') +
                '</textarea></div>' +
                '<div class="option-list">' +
                optionCheckbox(
                    'regex-step-ci-' + index,
                    !!step.caseInsensitive,
                    '大文字小文字を無視'
                ) +
                optionCheckbox('regex-step-ml-' + index, !!step.multiline, '複数行モード') +
                '</div></div>'
            );
        })
        .join('');

    return (
        '<div class="regex-presets">' +
        wrappedTable(
            '<table class="list-table"><thead><tr><th>有効</th><th>名前</th><th></th></tr></thead><tbody>' +
                presetRows +
                '</tbody></table>'
        ) +
        '<div class="card-actions toolbar">' +
        '<button type="button" class="ghost-btn" id="regex-preset-add-btn">プリセットを追加</button>' +
        '<span class="hint">最大 ' +
        maxRegexPresets() +
        ' 件</span></div></div>' +
        '<div class="field"><label for="regex-preset-name">プリセット名</label>' +
        '<input type="text" id="regex-preset-name" value="' +
        escapeHtml(selected.name || '') +
        '"></div>' +
        '<div class="regex-steps">' +
        (stepRows || '<div class="empty-hint">ステップがありません</div>') +
        '</div>' +
        '<div class="card-actions toolbar">' +
        '<button type="button" class="ghost-btn" id="regex-step-add-btn">ステップを追加</button>' +
        '<span class="hint">最大 ' +
        maxRegexStepsPerPreset() +
        ' ステップ</span></div>' +
        '<p id="regex-status" class="regex-status" role="status" aria-live="polite"></p>'
    );
}

/**
 * 新しいプリセット ID を生成する
 *
 * @returns {string}
 */
export function newPresetId() {
    return 'preset-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 8);
}

/**
 * 配列内の要素を入れ替える
 *
 * @template T
 * @param {T[]} list
 * @param {number} index
 * @param {'up'|'down'} direction
 * @returns {T[]}
 */
export function moveListItem(list, index, direction) {
    const next = list.slice();
    const target = direction === 'up' ? index - 1 : index + 1;
    if (index < 0 || target < 0 || target >= next.length) {
        return next;
    }
    const tmp = next[index];
    next[index] = next[target];
    next[target] = tmp;
    return next;
}
