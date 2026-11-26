import { defineConfig } from '@playwright/test';

export default defineConfig({
    testDir: './specs',
    testMatch: /.*\.spec\.ts/,
    globalSetup: './support/global_setup.ts',
    fullyParallel: false,
    workers: 1,
    timeout: 90_000,
    expect: {
        timeout: 15_000,
    },
    reporter: [['list']],
    // WebView2 は headed のみ (ヘッドレス非対応)
    use: {
        // 組み込みの trace は CDP 接続のコンテキストに効かないため off。失敗時のトレースは fixtures.ts の harness が保存する
        trace: 'off',
        video: 'off',
    },
});
