// ======================================================================
// 設定画面 HTML 組み立てヘルパー
// ======================================================================

import { ctx, loadedState } from './ctx.ts';

import SettingsSharedLogic from './shared_logic.ts';

export const TABS: Array<{ id: string; label: string; description: string }> = [
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
 */
export function escapeHtml(text: string): string {
    return SettingsSharedLogic.escapeHtml(text);
}

/**
 * タブ ID からタブメタデータを取得する
 */
function tabMeta(tabId: string): { id: string; label: string; description: string } {
    return (
        TABS.find((tab) => tab.id === tabId) ??
        TABS[0] ?? { id: tabId, label: tabId, description: '' }
    );
}

/**
 * パネル外枠 HTML を組み立てる
 */
export function panelWrap(tabId: string, bodyHtml: string): string {
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
 */
export function card(title: string, bodyHtml: string): string {
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
 * @param summary - 見出し
 * @param bodyHtml - 展開時に表示するカード群
 * @param open - 展開状態で描画するか
 */
export function advancedSection(summary: string, bodyHtml: string, open: boolean): string {
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
 */
export function optionCheckbox(
    id: string,
    checked: boolean,
    label: string,
    description?: string
): string {
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
 */
export function wrappedTable(tableHtml: string): string {
    return '<div class="list-table-wrap">' + tableHtml + '</div>';
}

/**
 * モード ID から表示ラベルを取得する
 */
export function modeLabel(modeId: string): string {
    const mode = loadedState().modes.find((entry) => entry.id === modeId);
    return mode ? mode.label : modeId;
}

/**
 * カテゴリ別 `<optgroup>` 付きモード選択 `<option>` HTML を組み立てる
 */
export function buildModeOptions(selectedId: string): string {
    const categories: string[] = [];
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
