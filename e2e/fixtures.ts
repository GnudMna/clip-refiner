import { test as base, expect, chromium, type Browser, type Page } from '@playwright/test';
import { ChildProcessWithoutNullStreams, spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const READY_MARKER = 'E2E_READY';

/** 1x1 透明 PNG (サムネ / ホバープレビュー用) */
export const TINY_PNG_DATA_URL =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

/**
 * E2E 用 ClipRefiner.exe の既定パスを解決する
 */
function defaultExePath(): string {
  if (process.env.CARGO_TARGET_DIR) {
    return path.resolve(process.env.CARGO_TARGET_DIR, 'debug', 'ClipRefiner.exe');
  }
  return path.resolve(process.cwd(), 'target', 'debug', 'ClipRefiner.exe');
}

export type E2eWindow = 'settings' | 'quick' | 'clip';

export type HarnessOptions = {
  /** `config.toml` の `favorite_modes` (PascalCase)。未指定時は Trim → UrlDecode */
  favoriteModes?: string[];
};

type HarnessFixtures = {
  e2eWindow: E2eWindow;
  harnessOptions: HarnessOptions;
  harness: RunningHarness;
  page: Page;
  configDir: string;
};

type RunningHarness = {
  browser: Browser;
  page: Page;
  child: ChildProcessWithoutNullStreams;
  configDir: string;
};

/**
 * 空き TCP ポートを1つ確保する
 */
async function findFreePort(): Promise<number> {
  return await new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (!address || typeof address === 'string') {
        server.close();
        reject(new Error('空きポートの取得に失敗'));
        return;
      }
      const { port } = address;
      server.close((err) => {
        if (err) {
          reject(err);
          return;
        }
        resolve(port);
      });
    });
    server.on('error', reject);
  });
}

/**
 * E2E 用の一時設定ディレクトリと初期 config.toml を用意する
 *
 * ホットキーは本番デフォルト (`consts::DEFAULT_HOTKEY_*`) に合わせる
 */
function prepareConfigDir(options: HarnessOptions = {}): string {
  const configDir = fs.mkdtempSync(path.join(os.tmpdir(), 'clip-refiner-e2e-'));
  const favoriteModes = options.favoriteModes ?? ['Trim', 'UrlDecode'];
  const favoriteModesToml = favoriteModes.map((mode) => `"${mode}"`).join(', ');
  const configToml = `# ClipRefiner E2E fixture
version = 3
interval_ms = 1000
mode = "UrlDecode"
is_paused = false
monitor_mode = "Event"
history_enabled = true
history_limit = 10
continuous_record_enabled = false
continuous_record_limit = 10
favorite_modes = [${favoriteModesToml}]

[regex]
active_preset_id = "default"
presets = [
  { id = "default", name = "デフォルト", steps = [
    { pattern = "foo", replacement = "bar", case_insensitive = false, multiline = false },
  ]},
]

[hotkeys]
quick_selector = "Alt+Shift+S"
clip_selector = "Alt+Shift+T"
pause = "Alt+Shift+P"
undo = "Alt+Shift+Z"
notification = "Alt+Shift+N"
continuous_record = "Alt+Shift+C"
continuous_record_undo = "Alt+Shift+Backspace"
ocr = "Alt+Shift+G"
quit = "Alt+Shift+Q"
`;
  fs.writeFileSync(path.join(configDir, 'config.toml'), configToml, 'utf8');
  return configDir;
}

/**
 * 子プロセスの stdout にマーカーが出るまで待つ
 */
async function waitForReady(
  child: ChildProcessWithoutNullStreams,
  timeoutMs: number,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error(`${READY_MARKER} 待ちがタイムアウトしました`));
    }, timeoutMs);

    const onData = (chunk: Buffer) => {
      const text = chunk.toString('utf8');
      if (text.includes(READY_MARKER)) {
        cleanup();
        resolve();
      }
    };
    const onExit = (code: number | null) => {
      cleanup();
      reject(new Error(`E2E プロセスが準備前に終了しました (code=${code})`));
    };
    const cleanup = () => {
      clearTimeout(timer);
      child.stdout.off('data', onData);
      child.off('exit', onExit);
    };

    child.stdout.on('data', onData);
    child.on('exit', onExit);
  });
}

/**
 * CDP 接続をリトライする
 */
async function connectWithRetry(port: number, timeoutMs: number): Promise<Browser> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown;
  while (Date.now() < deadline) {
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch (err) {
      lastError = err;
      await new Promise((r) => setTimeout(r, 250));
    }
  }
  throw new Error(`CDP 接続に失敗: ${String(lastError)}`);
}

/**
 * ClipRefiner E2E ハーネスを起動し Playwright Page を返す
 */
export async function startHarness(
  window: E2eWindow,
  options: HarnessOptions = {},
): Promise<RunningHarness> {
  const exe = process.env.CLIP_REFINER_E2E_EXE || defaultExePath();
  if (!fs.existsSync(exe)) {
    throw new Error(
      `E2E 用バイナリが見つかりません: ${exe}\n先に cargo build -p clip-refiner を実行してください`,
    );
  }

  const port = await findFreePort();
  const configDir = prepareConfigDir(options);
  const child = spawn(
    exe,
    ['--e2e-window', window],
    {
      env: {
        ...process.env,
        CLIP_REFINER_CONFIG_DIR: configDir,
        CLIP_REFINER_E2E_CDP_PORT: String(port),
        RUST_LOG: 'warn',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: false,
    },
  );

  child.stderr.on('data', (chunk: Buffer) => {
    process.stderr.write(chunk);
  });

  try {
    await waitForReady(child, 60_000);
    const browser = await connectWithRetry(port, 30_000);
    const context = browser.contexts()[0];
    if (!context) {
      throw new Error('CDP コンテキストがありません');
    }
    let page = context.pages()[0];
    if (!page) {
      page = await context.waitForEvent('page', { timeout: 15_000 });
    }
    // 初期の空ページ (about:blank) のまま返すと、直後の埋め込み HTML 読み込みで
    // テスト冒頭の evaluate が破棄されるため、埋め込み HTML の読み込み完了まで待つ
    await page.waitForFunction(
      () => document.readyState === 'complete' && document.body?.childElementCount > 0,
      undefined,
      { timeout: 15_000 },
    );
    return { browser, page, child, configDir };
  } catch (err) {
    if (!child.killed) {
      child.kill();
    }
    fs.rmSync(configDir, { recursive: true, force: true });
    throw err;
  }
}

/**
 * ハーネスを終了し一時ディレクトリを削除する
 */
export async function stopHarness(harness: RunningHarness): Promise<void> {
  try {
    await harness.browser.close();
  } catch {
    // 既に切断済みでもよい
  }
  if (!harness.child.killed) {
    harness.child.kill();
    await new Promise<void>((resolve) => {
      const timer = setTimeout(resolve, 5_000);
      harness.child.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }
  fs.rmSync(harness.configDir, { recursive: true, force: true });
}

/**
 * 一時設定ディレクトリの `config.toml` を読む
 *
 * 原子置換の瞬間にファイルが無い場合は空文字を返し、`expect.poll` 側で再試行させる
 */
export function readConfigToml(configDir: string): string {
  try {
    return fs.readFileSync(path.join(configDir, 'config.toml'), 'utf8');
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === 'ENOENT') {
      return '';
    }
    throw err;
  }
}

/**
 * 一時設定ディレクトリの `config.toml` を書き換える
 */
export function writeConfigToml(configDir: string, contents: string): void {
  fs.writeFileSync(path.join(configDir, 'config.toml'), contents, 'utf8');
}

/**
 * `favorite_modes = [...]` 行からモード ID 配列を取り出す
 */
export function parseFavoriteModes(configToml: string): string[] {
  const match = configToml.match(/favorite_modes\s*=\s*\[([^\]]*)\]/);
  if (!match) {
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

export const test = base.extend<HarnessFixtures>({
  e2eWindow: ['settings', { option: true }],
  harnessOptions: [{}, { option: true }],
  harness: async ({ e2eWindow, harnessOptions }, use) => {
    const harness = await startHarness(e2eWindow, harnessOptions);
    try {
      await use(harness);
    } finally {
      await stopHarness(harness);
    }
  },
  page: async ({ harness }, use) => {
    await use(harness.page);
  },
  configDir: async ({ harness }, use) => {
    await use(harness.configDir);
  },
});

export { expect };

/**
 * 埋め込み HTML の CSP が WebView 上に残っていることを検証する
 */
export async function expectWebViewCsp(page: Page): Promise<void> {
  const csp = page.locator('meta[http-equiv="Content-Security-Policy"]');
  await expect(csp).toHaveAttribute('content', /default-src 'none'/);
  await expect(csp).toHaveAttribute('content', /img-src data:/);
  await expect(csp).toHaveAttribute('content', /script-src 'sha256-/);
}
