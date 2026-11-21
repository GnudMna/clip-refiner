// ======================================================================
// 設定画面 共有ミュータブル状態
// ======================================================================

/**
 * 設定 UI 全体で共有するミュータブル状態
 *
 * ESM では import 束縛を再代入できないため、再代入が必要な値はここに集約する
 */
export const ctx = {
    /** @type {Record<string, unknown>|null} */
    state: null,
    /** @type {string} */
    activeTab: 'general',
    /** @type {ReturnType<typeof setTimeout>|null} */
    applyTimer: null,
    /** @type {ReturnType<typeof setTimeout>|null} */
    applyFeedbackTimer: null,
    /** @type {Record<string, unknown>} */
    pendingPatch: {},
    /** @type {boolean} */
    clipRegisterPending: false,
    /** @type {ReturnType<typeof setTimeout>|null} */
    regexFeedbackTimer: null,
    /** @type {number} */
    regexPreviewRequestId: 0,
    /** @type {number} */
    latestRegexPreviewId: 0,
    /**
     * @type {import('./regex_preview').RegexPreviewPending|null}
     */
    pendingRegexPreviewRequest: null,
    /** @type {import('./regex_preview').RegexPreviewCache} */
    lastRegexPreview: { settings: null, valid: true, message: '' },
    /** @type {(() => void)|null} */
    confirmDialogOnConfirm: null,
    /**
     * 編集ダイアログで開いている登録クリップ (閉じている間は `null`)
     *
     * `label` / `preview` は開いた時点の値で、一覧が他の操作で変わったかの判定に使う
     *
     * @type {{ index: number, label: string, preview: string, saving: boolean }|null}
     */
    clipEditor: null,
    /** @type {Record<string, number>} */
    tabScrollPositions: {},
    /** @type {boolean} 一般タブの詳細設定を展開しているか (再描画後も維持する) */
    advancedSectionOpen: false,
    /** @type {Set<string>} */
    renderedPanels: new Set(),
    /** @type {Set<string>} */
    boundPanels: new Set(),
    /** @type {boolean} */
    sidebarInitialized: false,
    /** @type {boolean} */
    contentDelegationBound: false,
    /** @type {boolean} */
    selectedRegexPresetId: null,
    /**
     * 直近の更新確認 / 適用結果
     *
     * @type {Record<string, unknown>|null}
     */
    updateStatus: null,
    /**
     * サイドバーとアクティブパネルを描画する
     *
     * `main.js` が初期化時に代入する
     *
     * @type {((forcePanel?: boolean) => void)|null}
     */
    render: null,
    /**
     * アクティブタブのパネルを破棄して再描画する
     *
     * `main.js` が初期化時に代入する
     *
     * @type {(() => void)|null}
     */
    refreshActivePanel: null,
};
