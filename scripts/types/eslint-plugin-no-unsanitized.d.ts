// 型定義を同梱しないプラグインの宣言 (`eslint.config.ts` の型検査用)
declare module 'eslint-plugin-no-unsanitized' {
    import type { ESLint } from 'eslint';

    const plugin: ESLint.Plugin;
    export default plugin;
}
