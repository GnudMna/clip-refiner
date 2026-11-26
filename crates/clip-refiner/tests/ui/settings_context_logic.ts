/**
 * 設定画面の共有状態 (`ctx`) と、それを参照する DOM 非依存モジュールの Node テスト
 */
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { ctx, loadedState } from '../../src/ui/settings/ctx.ts';
import {
    TABS,
    advancedSection,
    buildModeOptions,
    card,
    escapeHtml,
    modeLabel,
    optionCheckbox,
    panelWrap,
    wrappedTable,
} from '../../src/ui/settings/html_builders.ts';
import * as limits from '../../src/ui/settings/limits.ts';
import { validateRegexSettings } from '../../src/ui/settings/regex/regex_validation.ts';

import type { SettingsLimits, SettingsSnapshot } from '../../src/ui/ipc.ts';
import type { RegexSettings } from '../../src/ui/settings/regex/regex_preview.ts';

beforeEach(() => {
    ctx.state = null;
    ctx.activeTab = 'general';
    ctx.lastRegexPreview = { settings: null, valid: true, message: '' };
});

/**
 * モード一覧だけを持つ設定スナップショットを読み込み済みにする
 */
function loadModes(): void {
    ctx.state = {
        modes: [
            { id: 'trim', label: '前後空白除去', category: 'テキスト' },
            { id: 'url', label: 'URL <デコード>', category: 'A&B' },
            { id: 'json', label: 'JSON 整形', category: 'テキスト' },
        ],
    } as SettingsSnapshot;
}

/** 設定が未読み込みなら例外にし、読み込み後はスナップショットを返すこと */
test('loaded_state', () => {
    assert.throws(() => loadedState(), /未読み込み/);
    loadModes();
    assert.equal(loadedState(), ctx.state);
});

/** 描画関数は初期化前に呼ぶと例外になること */
test('render_before_initialization', () => {
    assert.throws(() => ctx.render(), /未初期化/);
    assert.throws(() => ctx.refreshActivePanel(), /未初期化/);
});

/** HTML 特殊文字をエスケープすること */
test('escape_html', () => {
    assert.equal(escapeHtml('<a>&"'), '&lt;a&gt;&amp;&quot;');
});

/** アクティブなタブのパネルだけを表示し、タブの見出しと説明を載せること */
test('panel_wrap', () => {
    const general = TABS.find((tab) => tab.id === 'general');
    assert.ok(general);

    const active = panelWrap('general', '<p>本文</p>');
    assert.ok(active.startsWith('<div class="panel active" id="panel-general"'));
    assert.ok(active.includes('aria-labelledby="tab-general"'));
    assert.ok(!active.includes(' hidden'));
    assert.ok(active.includes('>' + general.label + '</h2>'));
    assert.ok(active.includes('>' + general.description + '</p>'));
    assert.ok(active.endsWith('<p>本文</p></div>'));

    const inactive = panelWrap('monitor', '');
    assert.ok(inactive.startsWith('<div class="panel" id="panel-monitor"'));
    assert.ok(inactive.includes(' hidden>'));
});

/** 未知のタブ ID では先頭タブの見出しを使うこと */
test('panel_wrap_unknown_tab', () => {
    const html = panelWrap('unknown', '');
    assert.ok(html.includes('id="panel-unknown"'));
    assert.ok(html.includes('>' + TABS[0]?.label + '</h2>'));
});

/** カードの見出しだけをエスケープし、本文は HTML のまま載せること */
test('card', () => {
    const html = card('<見出し>', '<p>本文</p>');
    assert.ok(html.includes('&lt;見出し&gt;'));
    assert.ok(html.includes('<div class="card-body"><p>本文</p></div>'));
});

/** 詳細設定を指定した展開状態で描画すること */
test('advanced_section', () => {
    const open = advancedSection('<詳細>', '<p>本文</p>', true);
    assert.ok(open.startsWith('<details class="advanced-section" open>'));
    assert.ok(open.includes('<summary>&lt;詳細&gt;</summary>'));
    assert.ok(open.includes('<p>本文</p>'));

    const closed = advancedSection('<詳細>', '', false);
    assert.ok(closed.startsWith('<details class="advanced-section">'));
});

/** チェック状態と説明の有無を反映し、値をエスケープすること */
test('option_checkbox', () => {
    const checked = optionCheckbox('a"b', true, '<名前>', '<説明>');
    assert.ok(checked.includes('id="a&quot;b" checked>'));
    assert.ok(checked.includes('&lt;名前&gt;'));
    assert.ok(checked.includes('<span class="option-desc">&lt;説明&gt;</span>'));

    const unchecked = optionCheckbox('id', false, '名前');
    assert.ok(unchecked.includes('id="id">'));
    assert.ok(!unchecked.includes('option-desc'));
});

/** テーブルを横スクロール用の要素で包むこと */
test('wrapped_table', () => {
    assert.equal(
        wrappedTable('<table></table>'),
        '<div class="list-table-wrap"><table></table></div>'
    );
});

/** モードの表示ラベルを返し、未知の ID はそのまま返すこと */
test('mode_label', () => {
    loadModes();
    assert.equal(modeLabel('json'), 'JSON 整形');
    assert.equal(modeLabel('unknown'), 'unknown');
});

/** モードを出現順のカテゴリにまとめ、選択中のモードに印を付けること */
test('build_mode_options', () => {
    loadModes();
    assert.equal(
        buildModeOptions('json'),
        '<optgroup label="テキスト">' +
            '<option value="trim">前後空白除去</option>' +
            '<option value="json" selected>JSON 整形</option>' +
            '</optgroup>' +
            '<optgroup label="A&amp;B">' +
            '<option value="url">URL &lt;デコード&gt;</option>' +
            '</optgroup>'
    );
});

/**
 * すべての制限値を取得する
 */
function readLimits() {
    return {
        history: limits.historyLimitBounds(),
        record: limits.recordLimitBounds(),
        intervalMs: limits.intervalMsBounds(),
        selectorIdleMinutes: limits.selectorWebviewIdleMinutesBounds(),
        pipelineLength: limits.maxPipelineLength(),
        favoriteModes: limits.maxFavoriteModes(),
        registeredClips: limits.maxRegisteredClips(),
        registeredClipLabelChars: limits.maxRegisteredClipLabelChars(),
        regexPatternBytes: limits.maxRegexPatternBytes(),
        regexPresets: limits.maxRegexPresets(),
        regexStepsPerPreset: limits.maxRegexStepsPerPreset(),
    };
}

/** 設定が未読み込みなら既定の制限値を使うこと */
test('limits_default', () => {
    assert.deepEqual(readLimits(), {
        history: { min: 1, max: 100 },
        record: { min: 1, max: 100 },
        intervalMs: { min: 100, max: 60_000 },
        selectorIdleMinutes: { min: 1, max: 1440 },
        pipelineLength: 10,
        favoriteModes: 20,
        registeredClips: 100,
        registeredClipLabelChars: 64,
        regexPatternBytes: 8192,
        regexPresets: 20,
        regexStepsPerPreset: 10,
    });
});

/** 読み込んだスナップショットの制限値を優先すること */
test('limits_from_snapshot', () => {
    // 対応の取り違えを検出できるよう、すべて異なる値にする
    const snapshot: SettingsLimits = {
        historyMin: 2,
        historyMax: 3,
        continuousRecordMin: 4,
        continuousRecordMax: 5,
        intervalMinMs: 6,
        intervalMaxMs: 7,
        selectorWebviewIdleMinMinutes: 8,
        selectorWebviewIdleMaxMinutes: 9,
        maxPipelineLength: 11,
        maxFavoriteModes: 12,
        maxRegisteredClips: 13,
        maxRegisteredClipLabelChars: 14,
        maxRegexPatternBytes: 15,
        maxRegexPresets: 16,
        maxRegexStepsPerPreset: 17,
    };
    ctx.state = { limits: snapshot } as SettingsSnapshot;
    assert.deepEqual(readLimits(), {
        history: { min: 2, max: 3 },
        record: { min: 4, max: 5 },
        intervalMs: { min: 6, max: 7 },
        selectorIdleMinutes: { min: 8, max: 9 },
        pipelineLength: 11,
        favoriteModes: 12,
        registeredClips: 13,
        registeredClipLabelChars: 14,
        regexPatternBytes: 15,
        regexPresets: 16,
        regexStepsPerPreset: 17,
    });
});

/** 読み込んだ制限値と直近のプレビュー応答で正規表現設定を検証すること */
test('validate_regex_settings', () => {
    const settings: RegexSettings = {
        activePresetId: 'a',
        presets: [
            {
                id: 'a',
                name: 'A',
                steps: [
                    { pattern: 'abc', replacement: '', caseInsensitive: false, multiline: false },
                ],
            },
        ],
    };
    assert.equal(validateRegexSettings(settings).status, 'pending');

    ctx.lastRegexPreview = { settings, valid: false, message: 'パターンが無効です' };
    assert.deepEqual(validateRegexSettings(settings), {
        valid: false,
        status: 'error',
        message: 'パターンが無効です',
    });

    ctx.state = { limits: { maxRegexPatternBytes: 2 } } as SettingsSnapshot;
    assert.match(validateRegexSettings(settings).message, /最大 2 バイト/);
});
