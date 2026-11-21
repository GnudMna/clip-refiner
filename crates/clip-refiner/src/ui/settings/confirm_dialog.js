// ======================================================================
// 設定画面 確認ダイアログ
// ======================================================================

/**
 * 確認ダイアログを表示する
 *
 * @param {{ message: string, confirmLabel?: string, onConfirm?: () => void }} options
 */
function showConfirmDialog(options) {
    const dialog = document.getElementById('confirm-dialog');
    const messageEl = document.getElementById('confirm-dialog-message');
    const okBtn = document.getElementById('confirm-dialog-ok');
    if (!dialog || !messageEl || !okBtn) {
        return;
    }

    messageEl.textContent = options.message;
    okBtn.textContent = options.confirmLabel || '確認';
    confirmDialogOnConfirm = options.onConfirm || null;
    dialog.hidden = false;
    okBtn.focus();
}

/** 確認ダイアログを非表示にする */
function hideConfirmDialog() {
    const dialog = document.getElementById('confirm-dialog');
    if (dialog) {
        dialog.hidden = true;
    }
    confirmDialogOnConfirm = null;
}

/** 確認ダイアログのボタン・Escape キーをバインドする */
function bindConfirmDialog() {
    const dialog = document.getElementById('confirm-dialog');
    const okBtn = document.getElementById('confirm-dialog-ok');
    if (!dialog || !okBtn) {
        return;
    }

    okBtn.addEventListener('click', () => {
        const onConfirm = confirmDialogOnConfirm;
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
