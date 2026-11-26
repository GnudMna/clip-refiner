// ======================================================================
// 設定画面 確認ダイアログ
// ======================================================================

import { ctx } from '../ctx.ts';

/**
 * 確認ダイアログを表示する
 */
export function showConfirmDialog(options: {
    message: string;
    confirmLabel?: string;
    onConfirm?: () => void;
}): void {
    const dialog = document.getElementById('confirm-dialog');
    const messageEl = document.getElementById('confirm-dialog-message');
    const okBtn = document.getElementById('confirm-dialog-ok');
    if (!dialog || !messageEl || !okBtn) {
        return;
    }

    messageEl.textContent = options.message;
    okBtn.textContent = options.confirmLabel || '確認';
    ctx.confirmDialogOnConfirm = options.onConfirm || null;
    dialog.hidden = false;
    okBtn.focus();
}

/** 確認ダイアログを非表示にする */
function hideConfirmDialog(): void {
    const dialog = document.getElementById('confirm-dialog');
    if (dialog) {
        dialog.hidden = true;
    }
    ctx.confirmDialogOnConfirm = null;
}

/** 確認ダイアログのボタン・Escape キーをバインドする */
export function bindConfirmDialog(): void {
    const dialog = document.getElementById('confirm-dialog');
    const okBtn = document.getElementById('confirm-dialog-ok');
    if (!dialog || !okBtn) {
        return;
    }

    okBtn.addEventListener('click', () => {
        const onConfirm = ctx.confirmDialogOnConfirm;
        hideConfirmDialog();
        onConfirm?.();
    });

    dialog.querySelectorAll('[data-confirm-cancel]').forEach((el) => {
        el.addEventListener('click', hideConfirmDialog);
    });

    document.addEventListener('keydown', (event) => {
        if (dialog.hidden) {
            return;
        }
        if (event.key === 'Escape') {
            event.preventDefault();
            hideConfirmDialog();
        }
    });
}
