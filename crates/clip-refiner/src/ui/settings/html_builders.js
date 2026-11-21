// ======================================================================
// 設定画面 HTML 組み立てヘルパー
// ======================================================================

/** @type {Array<{ id: string, label: string, description: string }>} */
const TABS = [
    {
        id: 'general',
        label: '一般',
        description: '起動時の動作などアプリ全体の設定を管理する',
    },
    {
        id: 'favorites',
        label: 'お気に入り',
        description: 'クイックセレクターとホットキーで使う変換モードを登録する',
    },
    {
        id: 'regex',
        label: '正規表現',
        description: '正規表現モード用のパターンと置換文字列を編集する',
    },
    {
        id: 'history',
        label: '履歴・連続記録',
        description: 'クリップボード履歴と連続記録の保持設定を管理する',
    },
    {
        id: 'clips',
        label: '登録クリップ',
        description: 'よく使うテキストや画像を登録してワンタッチでコピーする',
    },
    {
        id: 'monitor',
        label: '監視',
        description: 'クリップボード監視の方式・周期・加工パイプラインを設定する',
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
];

/**
 * HTML 特殊文字をエスケープする
 *
 * @param {string} text
 * @returns {string}
 */
function escapeHtml(text) {
    return SettingsSharedLogic.escapeHtml(text);
}

/**
 * タブ ID からタブメタデータを取得する
 *
 * @param {string} tabId
 * @returns {{ id: string, label: string, description: string }}
 */
function tabMeta(tabId) {
    return TABS.find((tab) => tab.id === tabId) || TABS[0];
}

/**
 * パネル外枠 HTML を組み立てる
 *
 * @param {string} tabId
 * @param {string} bodyHtml
 * @returns {string}
 */
function panelWrap(tabId, bodyHtml) {
    const tab = tabMeta(tabId);
    const activeClass = tabId === activeTab ? ' active' : '';
    return (
        '' +
        '<div class="panel' +
        activeClass +
        '" id="panel-' +
        tabId +
        '">' +
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
function card(title, bodyHtml) {
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
 * オプションカード形式のチェックボックス HTML を組み立てる
 *
 * @param {string} id
 * @param {boolean} checked
 * @param {string} label
 * @param {string} [description]
 * @returns {string}
 */
function optionCheckbox(id, checked, label, description) {
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
function wrappedTable(tableHtml) {
    return '<div class="list-table-wrap">' + tableHtml + '</div>';
}

/**
 * モード ID から表示ラベルを取得する
 *
 * @param {string} modeId
 * @returns {string}
 */
function modeLabel(modeId) {
    const mode = state.modes.find((entry) => entry.id === modeId);
    return mode ? mode.label : modeId;
}

/**
 * カテゴリ別 `<optgroup>` 付きモード選択 `<option>` HTML を組み立てる
 *
 * @param {string} selectedId
 * @returns {string}
 */
function buildModeOptions(selectedId) {
    const categories = [];
    for (const mode of state.modes) {
        if (!categories.includes(mode.category)) {
            categories.push(mode.category);
        }
    }
    let html = '';
    for (const category of categories) {
        html += '<optgroup label="' + escapeHtml(category) + '">';
        for (const mode of state.modes.filter((entry) => entry.category === category)) {
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
