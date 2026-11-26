// ======================================================================
// 設定画面 登録クリップ編集ダイアログ
// ======================================================================

import { ctx } from '../ctx.ts';
import { postIpcEnvelope, showApplyFeedback } from '../ipc_apply.ts';
import { maxRegisteredClipLabelChars } from '../limits.ts';

import SettingsPanelsLogic from '../panels_logic.ts';

import type { ClipSelectorItem } from '../../selector/clip_selector_logic.ts';
import type { ClipEditorPayload, ClipEditResult } from '../../ipc.ts';

/** 編集ダイアログ */
function dialogEl(): HTMLElement | null {
    return document.getElementById('clip-editor');
}

/** ラベル入力欄 */
function labelInputEl(): HTMLInputElement | null {
    return document.getElementById('clip-editor-label') as HTMLInputElement | null;
}

/** 本文入力欄 */
function textInputEl(): HTMLTextAreaElement | null {
    return document.getElementById('clip-editor-text') as HTMLTextAreaElement | null;
}

/**
 * 編集ダイアログが開いているか
 */
export function isClipEditorOpen(): boolean {
    const dialog = dialogEl();
    return Boolean(dialog && !dialog.hidden);
}

/**
 * ダイアログ内のエラー表示を更新する
 *
 * @param message - 空文字で非表示にする
 */
function setEditorError(message: string): void {
    const el = document.getElementById('clip-editor-error');
    if (!el) {
        return;
    }
    el.textContent = message;
    el.hidden = message === '';
}

/**
 * 保存ボタンの表示を保存中の状態に合わせる
 */
function setSavingState(saving: boolean): void {
    if (ctx.clipEditor) {
        ctx.clipEditor.saving = saving;
    }
    const button = document.getElementById('clip-editor-save') as HTMLButtonElement | null;
    if (button) {
        button.disabled = saving;
        button.textContent = saving ? '保存中...' : '保存';
    }
}

/**
 * 編集ダイアログを開く
 *
 * Rust から `window.showClipEditor(payload)` として呼び出す
 */
export function showClipEditor(payload: ClipEditorPayload): void {
    const dialog = dialogEl();
    const labelInput = labelInputEl();
    const textInput = textInputEl();
    const clip = ctx.state?.clips?.[payload?.index];
    if (!dialog || !labelInput || !textInput || !clip || clip.kind !== 'text') {
        return;
    }
    // 本文を読み出した後に一覧が変わっていた場合は、別のクリップを開かない
    if (clip.token !== payload.token) {
        return;
    }

    ctx.clipEditor = {
        index: payload.index,
        token: payload.token,
        label: clip.label ?? '',
        preview: clip.preview ?? '',
        saving: false,
    };
    labelInput.value = payload.label ?? '';
    textInput.value = payload.text ?? '';
    setEditorError('');
    setSavingState(false);
    dialog.hidden = false;
    textInput.focus();
    textInput.setSelectionRange(0, 0);
    textInput.scrollTop = 0;
}

/** 編集ダイアログを閉じ、入力欄の本文を消す */
function hideClipEditor(): void {
    const dialog = dialogEl();
    if (dialog) {
        dialog.hidden = true;
    }
    const labelInput = labelInputEl();
    const textInput = textInputEl();
    if (labelInput) {
        labelInput.value = '';
    }
    if (textInput) {
        textInput.value = '';
    }
    setEditorError('');
    ctx.clipEditor = null;
}

/**
 * 一覧が変わって編集対象がずれた場合にダイアログを閉じる
 *
 * 保存中は自分の保存で一覧が変わるため対象外にする
 */
export function closeClipEditorIfStale(clips: ClipSelectorItem[] | undefined): void {
    const editor = ctx.clipEditor;
    if (!editor || editor.saving) {
        return;
    }
    if (SettingsPanelsLogic.clipEditorTargetUnchanged(editor, clips)) {
        return;
    }
    hideClipEditor();
    showApplyFeedback({
        ok: false,
        message: '登録クリップが他の操作で変更されたため、編集を閉じました',
    });
}

/** 入力内容を検証して保存を要求する */
function saveClipEditor(): void {
    const editor = ctx.clipEditor;
    const labelInput = labelInputEl();
    const textInput = textInputEl();
    if (!editor || editor.saving || !labelInput || !textInput) {
        return;
    }

    const check = SettingsPanelsLogic.validateClipEdit(
        labelInput.value,
        textInput.value,
        maxRegisteredClipLabelChars()
    );
    if (check.ok === false) {
        setEditorError(check.message);
        (check.field === 'label' ? labelInput : textInput).focus();
        return;
    }

    setEditorError('');
    setSavingState(true);
    postIpcEnvelope({
        type: 'clip-edit',
        index: editor.index,
        token: editor.token,
        label: labelInput.value,
        text: textInput.value,
    });
}

/**
 * 登録クリップの編集結果を受け取る
 *
 * Rust から `window.onClipEditResult(result)` として呼び出す
 * 編集を始める前の失敗 (本文を読み出せないなど) はダイアログが無いためバナーで伝える
 */
export function onClipEditResult(result: ClipEditResult): void {
    const ok = result?.ok === true;
    if (!ok && isClipEditorOpen()) {
        setEditorError(result?.message || '登録クリップを保存できませんでした');
        setSavingState(false);
        return;
    }

    hideClipEditor();
    showApplyFeedback({
        ok,
        message: ok
            ? '登録クリップを更新しました'
            : result?.message || '登録クリップを開けませんでした',
    });
}

/**
 * ダイアログ内のフォーカスを循環させる
 */
function trapFocus(event: KeyboardEvent, dialog: HTMLElement): void {
    const focusable: HTMLElement[] = Array.from(
        dialog.querySelectorAll('input, textarea, button:not([disabled])')
    );
    if (focusable.length === 0) {
        return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) {
        return;
    }
    if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
    }
}

/** 編集ダイアログのボタン・キー操作をバインドする */
export function bindClipEditor(): void {
    const dialog = dialogEl();
    if (!dialog) {
        return;
    }

    document.getElementById('clip-editor-save')?.addEventListener('click', saveClipEditor);
    dialog.querySelectorAll('[data-clip-editor-cancel]').forEach((el) => {
        el.addEventListener('click', hideClipEditor);
    });

    // 後から登録される設定ウィンドウ全体の Escape 処理へ渡さない
    document.addEventListener('keydown', (event) => {
        if (dialog.hidden) {
            return;
        }
        if (event.key === 'Escape') {
            event.preventDefault();
            event.stopImmediatePropagation();
            hideClipEditor();
        } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            saveClipEditor();
        } else if (event.key === 'Tab') {
            trapFocus(event, dialog);
        }
    });
}
