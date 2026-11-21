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
     * @type {{ requestId: number, settings: import('./regex_preview').RegexSettings }|null}
     */
    pendingRegexPreviewRequest: null,
    /** @type {import('./regex_preview').RegexPreviewCache} */
    lastRegexPreview: { settings: null, valid: true, message: '' },
    /** @type {(() => void)|null} */
    confirmDialogOnConfirm: null,
    /** @type {Record<string, number>} */
    tabScrollPositions: {},
    /** @type {Set<string>} */
    renderedPanels: new Set(),
    /** @type {Set<string>} */
    boundPanels: new Set(),
    /** @type {boolean} */
    sidebarInitialized: false,
    /** @type {boolean} */
    contentDelegationBound: false,
    /** @type {boolean} */
    updateBusy: false,
    /**
     * 直近の更新確認 / 適用結果
     *
     * @type {Record<string, unknown>|null}
     */
    updateStatus: null,
};
