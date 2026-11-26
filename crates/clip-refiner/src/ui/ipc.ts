// ======================================================================
// WebView IPC の型定義
// ======================================================================
//
// Rust と WebView の間で受け渡す JSON の形を `tsc` で検証するための宣言
// 型だけを置く (`import type` で参照し、バンドルには含めない)
// Rust 側の型を変えたら、対応する宣言も同じ変更で更新する
// (ずれは `tray/settings/webview_contract_tests.rs` が `cargo test` で検出する)

import type { ClipSelectorItem } from './selector/clip_selector_logic.ts';
import type { QuickSelectorMode } from './selector/quick_selector_logic.ts';
import type { RegexPreviewPayload, RegexSettings } from './settings/regex/regex_preview.ts';

// ======================================================================
// 設定スナップショット (Rust → JS)
// ======================================================================

/** `MonitorMode` */
export type MonitorMode = 'Polling' | 'Event';

/** `SelectorWebviewRetention` */
export type SelectorWebviewRetention = 'Keep' | 'DisposeOnHide' | 'DisposeAfterIdle';

/** `NotificationSettings` */
export interface NotificationSettings {
    enabled: boolean;
    notifyProcess: boolean;
    notifyAutoRefine: boolean;
    notifyHistoryRestore: boolean;
    notifyOcr: boolean;
    notifyRegisteredClip: boolean;
    notifyFavorites: boolean;
    notifyContinuousRecord: boolean;
    notifyResult: boolean;
}

/** `HotkeySettings` */
export interface HotkeySettings {
    quickSelector: string;
    notification: string;
    autoRefine: string;
    quit: string;
    undo: string;
    clipSelector: string;
    ocr: string;
    continuousRecord: string;
    continuousRecordUndo: string;
    continuousRecordPaste: string;
    favoriteModeSlots: string[];
}

/** `SettingsWritable` (設定ウィンドウで読み書きする項目) */
interface SettingsWritable {
    monitorMode: MonitorMode;
    intervalMs: number;
    selectorWebviewRetention: SelectorWebviewRetention;
    selectorWebviewIdleMinutes: number;
    /** `RefineMode` の識別子 */
    mode: string;
    pipeline: string[];
    notificationSettings: NotificationSettings;
    historyEnabled: boolean;
    historyLimit: number;
    keepClipboardExclusion: boolean;
    continuousRecordEnabled: boolean;
    continuousRecordLimit: number;
    regex: RegexSettings;
    hotkeys: HotkeySettings;
    launchAtLogin: boolean;
    autoRefineEnabled: boolean;
}

/**
 * `SettingsPatch` (部分更新)
 *
 * 各項目は `#[serde(default)]` 付きのため、通知設定とホットキーは一部のキーだけ送ってよい
 */
export type SettingsPatch = Partial<Omit<SettingsWritable, 'notificationSettings' | 'hotkeys'>> & {
    notificationSettings?: Partial<NotificationSettings>;
    hotkeys?: Partial<HotkeySettings>;
};

/** `ShortcutEntry` */
interface ShortcutEntry {
    binding: string;
    label: string;
}

/** `SettingsLimits` */
export interface SettingsLimits {
    historyMin: number;
    historyMax: number;
    continuousRecordMin: number;
    continuousRecordMax: number;
    intervalMinMs: number;
    intervalMaxMs: number;
    selectorWebviewIdleMinMinutes: number;
    selectorWebviewIdleMaxMinutes: number;
    maxPipelineLength: number;
    maxFavoriteModes: number;
    maxRegisteredClips: number;
    maxRegisteredClipLabelChars: number;
    maxRegexPatternBytes: number;
    maxRegexPresets: number;
    maxRegexStepsPerPreset: number;
}

/** `SettingsSnapshot` (`loadSettings` の引数) */
export interface SettingsSnapshot extends SettingsWritable {
    appVersion: string;
    portableMode: boolean;
    regexPreviewSample: string;
    favoriteModes: string[];
    modes: QuickSelectorMode[];
    clips: ClipSelectorItem[];
    shortcuts: ShortcutEntry[];
    limits: SettingsLimits;
}

/** `loadSettings` のオプション */
export interface LoadSettingsOptions {
    force?: boolean;
    externalSync?: boolean;
}

/** `favorites_and_shortcuts_json` (`updateFavoritesAndShortcuts` の引数) */
export interface FavoritesAndShortcutsPayload {
    favoriteModes: string[];
    hotkeys: HotkeySettings;
    shortcuts: ShortcutEntry[];
}

/** `SettingsApplyFeedback` (`onApplyFeedback` の引数) */
export interface ApplyFeedback {
    ok?: boolean;
    field?: string;
    message?: string;
}

/** `showClipEditor` の引数 */
export interface ClipEditorPayload {
    index: number;
    token: string;
    label: string;
    text: string;
}

/** `onClipEditResult` の引数 */
export interface ClipEditResult {
    ok: boolean;
    message: string;
}

// ======================================================================
// 設定ウィンドウの IPC メッセージ (JS → Rust)
// ======================================================================

/** `IpcClipTarget` */
interface ClipTarget {
    index: number;
    /** 16 桁の 16 進数 */
    token: string;
}

/** `IpcDocumentTarget` */
export type DocumentTarget = 'user-docs' | 'license' | 'third-party-notices';

/** `IpcFavoriteMoveDirection` */
export type MoveDirection = 'up' | 'down';

/** `SettingsIpcMessage` */
export type SettingsIpcMessage =
    | { type: 'close' }
    | { type: 'open-config' }
    | { type: 'open-document'; target: DocumentTarget }
    | { type: 'apply'; patch: SettingsPatch }
    | { type: 'clear-history' }
    | { type: 'clear-record' }
    | ({ type: 'clip-copy' } & ClipTarget)
    | ({ type: 'clip-delete' } & ClipTarget)
    | ({ type: 'clip-move'; direction: MoveDirection } & ClipTarget)
    | ({ type: 'clip-template'; enabled: boolean } & ClipTarget)
    | ({ type: 'clip-counter-reset' } & ClipTarget)
    | ({ type: 'clip-edit-open' } & ClipTarget)
    | ({ type: 'clip-edit'; label: string; text: string } & ClipTarget)
    | { type: 'clip-register' }
    | { type: 'favorite-toggle'; mode: string }
    | { type: 'favorite-move'; mode: string; direction: MoveDirection }
    | { type: 'regex-preview'; requestId: number; settings: RegexSettings; sample?: string }
    | { type: 'check-update' };

// ======================================================================
// Rust から呼び出すグローバル関数
// ======================================================================

/** 登録クリップセレクターの `focusInput` */
export type ClipSelectorFocusInput = (items: ClipSelectorItem[]) => void;

/** クイックセレクターの `focusInput` (展開中カテゴリはアプリ側で保持した値) */
export type QuickSelectorFocusInput = (
    modeId: string | null,
    modes?: QuickSelectorMode[],
    expanded?: string[]
) => void;

declare global {
    interface Window {
        /** wry が注入する IPC */
        ipc: { postMessage(message: string): void };

        // 設定ウィンドウ
        loadSettings(snapshot: SettingsSnapshot, options?: LoadSettingsOptions): void;
        updateClips(clips: ClipSelectorItem[]): void;
        showClipEditor(payload: ClipEditorPayload): void;
        onClipEditResult(result: ClipEditResult): void;
        updateFavoritesAndShortcuts(payload: FavoritesAndShortcutsPayload): void;
        flushPendingApply(): void;
        closeWindow(): void;
        onApplyFeedback(feedback?: ApplyFeedback): void;
        onRegexPreview(payload: RegexPreviewPayload): void;

        // クイックセレクター / 登録クリップセレクター (同名で引数が異なるため画面別の型の交差にする)
        focusInput: ClipSelectorFocusInput & QuickSelectorFocusInput;
        refreshModes(modeId: string | null, modes: QuickSelectorMode[]): void;
        refreshItems(clips: ClipSelectorItem[]): void;
        showClipFullText(index: number, text: string): void;
    }
}
