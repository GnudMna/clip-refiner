// ======================================================================
// 設定画面 共有ミュータブル状態
// ======================================================================

import type { SettingsPatch, SettingsSnapshot } from '../ipc.ts';
import type { RegexPreviewCache, RegexPreviewPending } from './regex/regex_preview.ts';

/** 編集ダイアログで開いている登録クリップ (`label` / `preview` は開いた時点の値) */
interface ClipEditorState {
    index: number;
    token: string;
    label: string;
    preview: string;
    saving: boolean;
}

/** 設定 UI 全体で共有するミュータブル状態の型 */
interface SettingsContext {
    state: SettingsSnapshot | null;
    activeTab: string;
    applyTimer: ReturnType<typeof setTimeout> | null;
    applyFeedbackTimer: ReturnType<typeof setTimeout> | null;
    pendingPatch: SettingsPatch;
    clipRegisterPending: boolean;
    regexFeedbackTimer: ReturnType<typeof setTimeout> | null;
    regexPreviewRequestId: number;
    latestRegexPreviewId: number;
    pendingRegexPreviewRequest: RegexPreviewPending | null;
    lastRegexPreview: RegexPreviewCache;
    confirmDialogOnConfirm: (() => void) | null;
    /**
     * 編集ダイアログで開いている登録クリップ (閉じている間は `null`)
     *
     * `label` / `preview` は一覧が他の操作で変わったかの判定に使う
     */
    clipEditor: ClipEditorState | null;
    tabScrollPositions: Record<string, number>;
    /** 一般タブの詳細設定を展開しているか (再描画後も維持する) */
    advancedSectionOpen: boolean;
    renderedPanels: Set<string>;
    boundPanels: Set<string>;
    sidebarInitialized: boolean;
    contentDelegationBound: boolean;
    selectedRegexPresetId: string | null;
    /**
     * サイドバーとアクティブパネルを描画する
     *
     * `main.ts` が初期化時に代入する
     */
    render: (forcePanel?: boolean) => void;
    /**
     * アクティブタブのパネルを破棄して再描画する
     *
     * `main.ts` が初期化時に代入する
     */
    refreshActivePanel: () => void;
}

/**
 * `main.ts` が代入する前の描画関数
 */
function notInitialized(): never {
    throw new Error('設定 UI の描画関数が未初期化');
}

/**
 * 設定 UI 全体で共有するミュータブル状態
 *
 * ESM では import 束縛を再代入できないため、再代入が必要な値はここに集約する
 */
export const ctx: SettingsContext = {
    state: null,
    activeTab: 'general',
    applyTimer: null,
    applyFeedbackTimer: null,
    pendingPatch: {},
    clipRegisterPending: false,
    regexFeedbackTimer: null,
    regexPreviewRequestId: 0,
    latestRegexPreviewId: 0,
    pendingRegexPreviewRequest: null,
    lastRegexPreview: { settings: null, valid: true, message: '' },
    confirmDialogOnConfirm: null,
    clipEditor: null,
    tabScrollPositions: {},
    advancedSectionOpen: false,
    renderedPanels: new Set(),
    boundPanels: new Set(),
    sidebarInitialized: false,
    contentDelegationBound: false,
    selectedRegexPresetId: null,
    render: notInitialized,
    refreshActivePanel: notInitialized,
};

/**
 * 読み込み済みの設定スナップショットを返す
 *
 * パネルの描画とフォームのイベントは `loadSettings` の後にしか動かないため、
 * 未読み込みでの呼び出しは実装の誤りとして例外にする
 */
export function loadedState(): SettingsSnapshot {
    if (!ctx.state) {
        throw new Error('設定スナップショットが未読み込み');
    }
    return ctx.state;
}
