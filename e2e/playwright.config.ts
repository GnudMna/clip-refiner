import { defineConfig } from '@playwright/test';

export default defineConfig({
    testDir: '.',
    testMatch: /.*\.spec\.ts/,
    globalSetup: './global-setup.ts',
    fullyParallel: false,
    workers: 1,
    timeout: 90_000,
    expect: {
        timeout: 15_000,
    },
    reporter: [['list']],
    // WebView2 は headed のみ (ヘッドレス非対応)
    use: {
        trace: 'off',
        video: 'off',
    },
});
