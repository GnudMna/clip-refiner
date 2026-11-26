/**
 * クイックセレクター / 登録クリップセレクターで共有する IPC 送信
 */

/**
 * WebView IPC にメッセージを送信する
 */
export function postIpc(message: string): void {
    window.ipc.postMessage(message);
}

/**
 * セレクターウィンドウを閉じる
 */
export function closeSelectorWindow(): void {
    postIpc('close');
}
