// ======================================================================
// 設定画面 HTML 組み立てヘルパー
// ======================================================================

import { ctx, loadedState } from './ctx.js';

import SettingsSharedLogic from './shared_logic.js';

/** @type {Array<{ id: string, label: string, description: string }>} */
export const TABS = [
    {
        id: 'general',
        label: '一般',
        description: '起動時の動作・更新など、アプリ全体の設定を管理する',
    },
    {
        id: 'monitor',
        label: '監視',
        description: 'クリップボードの監視方式とポーリング間隔を設定する',
    },
    {
        id: 'convert',
        label: '変換',
        description: 'コピー時の自動変換・加工パイプライン・お気に入り変換モードを設定する',
    },
    {
        id: 'regex',
        label: '正規表現',
        description: '正規表現モード用のパターンと置換文字列を編集する',
    },
    {
        id: 'clipboard',
        label: 'クリップボード',
        description: '履歴・連続記録モード・登録クリップを管理する',
    },
    {
        id: 'notifications',
        label: '通知',
        description: 'デスクトップ通知の表示条件を設定する',
    },
    {
        id: 'shortcuts',
        label: 'ショートカット',
        description: 'グローバルホットキーを編集する',
    },
    {
        id: 'about',
        label: '情報',
        description: '利用者向けドキュメントと第三者ライセンスの表示を開く',
    },
];

/**
 * HTML 特殊文字をエスケープする
 *
 * @param {string} text
 * @returns {string}
 */
export function escapeHtml(text) {
    return SettingsSharedLogic.escapeHtml(text);
}

/**
 * タブ ID からタブメタデータを取得する
 *
 * @param {string} tabId
 * @returns {{ id: string, label: string, description: string }}
 */
export function tabMeta(tabId) {
    return TABS.find((tab) => tab.id === tabId) || TABS[0];
}

/**
 * パネル外枠 HTML を組み立てる
 *
 * @param {string} tabId
 * @param {string} bodyHtml
 * @returns {string}
 */
export function panelWrap(tabId, bodyHtml) {
    const tab = tabMeta(tabId);
    const isActive = tabId === ctx.activeTab;
    const activeClass = isActive ? ' active' : '';
    return (
        '' +
        '<div class="panel' +
        activeClass +
        '" id="panel-' +
        tabId +
        '" role="tabpanel" aria-labelledby="tab-' +
        tabId +
        '"' +
        (isActive ? '' : ' hidden') +
        '>' +
        '<header class="panel-intro">' +
        '<h2 class="panel-title">' +
        escapeHtml(tab.label) +
        '</h2>' +
        '<p class="panel-desc">' +
        escapeHtml(tab.description) +
        '</p>' +
        '</header>' +
        bodyHtml +
        '</div>'
    );
}

/**
 * カードセクション HTML を組み立てる
 *
 * @param {string} title
 * @param {string} bodyHtml
 * @returns {string}
 */
export function card(title, bodyHtml) {
    return (
        '' +
        '<section class="card">' +
        '<h3 class="card-title">' +
        escapeHtml(title) +
        '</h3>' +
        '<div class="card-body">' +
        bodyHtml +
        '</div>' +
        '</section>'
    );
}

/**
 * 折りたたみ表示の詳細設定セクション HTML を組み立てる
 *
 * @param {string} summary - 見出し
 * @param {string} bodyHtml - 展開時に表示するカード群
 * @param {boolean} open - 展開状態で描画するか
 * @returns {string}
 */
export function advancedSection(summary, bodyHtml, open) {
    return (
        '' +
        '<details class="advanced-section"' +
        (open ? ' open' : '') +
        '>' +
        '<summary>' +
        escapeHtml(summary) +
        '</summary>' +
        '<div class="advanced-section-body">' +
        bodyHtml +
        '</div>' +
        '</details>'
    );
}

/**
 * オプションカード形式のチェックボックス HTML を組み立てる
 *
 * @param {string} id
 * @param {boolean} checked
 * @param {string} label
 * @param {string} [description]
 * @returns {string}
 */
export function optionCheckbox(id, checked, label, description) {
    const descHtml = description
        ? '<span class="option-desc">' + escapeHtml(description) + '</span>'
        : '';
    return (
        '' +
        '<label class="option-card">' +
        '<input type="checkbox" id="' +
        escapeHtml(id) +
        '"' +
        (checked ? ' checked' : '') +
        '>' +
        '<span class="option-text">' +
        '<span class="option-label">' +
        escapeHtml(label) +
        '</span>' +
        descHtml +
        '</span></label>'
    );
}

/**
 * テーブルを横スクロール用ラッパーで包む
 *
 * @param {string} tableHtml
 * @returns {string}
 */
export function wrappedTable(tableHtml) {
    return '<div class="list-table-wrap">' + tableHtml + '</div>';
}

/**
 * モード ID から表示ラベルを取得する
 *
 * @param {string} modeId
 * @returns {string}
 */
export function modeLabel(modeId) {
    const mode = loadedState().modes.find((entry) => entry.id === modeId);
    return mode ? mode.label : modeId;
}

/**
 * カテゴリ別 `<optgroup>` 付きモード選択 `<option>` HTML を組み立てる
 *
 * @param {string} selectedId
 * @returns {string}
 */
export function buildModeOptions(selectedId) {
    /** @type {string[]} */
    const categories = [];
    for (const mode of loadedState().modes) {
        if (!categories.includes(mode.category)) {
            categories.push(mode.category);
        }
    }
    let html = '';
    for (const category of categories) {
        html += '<optgroup label="' + escapeHtml(category) + '">';
        for (const mode of loadedState().modes.filter((entry) => entry.category === category)) {
            const selected = mode.id === selectedId ? ' selected' : '';
            html +=
                '<option value="' +
                escapeHtml(mode.id) +
                '"' +
                selected +
                '>' +
                escapeHtml(mode.label) +
                '</option>';
        }
        html += '</optgroup>';
    }
    return html;
}
