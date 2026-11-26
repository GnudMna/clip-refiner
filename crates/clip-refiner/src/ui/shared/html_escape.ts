/**
 * セレクターと設定画面で共有する HTML エスケープ
 *
 * `innerHTML` へ渡す文字列のサニタイズ実装をここに一本化する
 */

/**
 * HTML 特殊文字をエスケープする
 */
export function escapeHtml(text: string): string {
    return String(text)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}
