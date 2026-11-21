import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

/** 前回までの実行で残ったとみなす経過時間 (ミリ秒。並行して動く実行のものを消さない程度に取る) */
const STALE_AGE_MS = 10 * 60 * 1000;

/** `%TEMP%` 直下で `match` に合うディレクトリを削除する (`olderThanMs` 指定時は古いものだけ) */
async function removeTempDirs(
  match: (name: string) => boolean,
  olderThanMs?: number,
): Promise<void> {
  const tmp = os.tmpdir();
  let names: string[];
  try {
    names = await fs.promises.readdir(tmp);
  } catch {
    return;
  }
  for (const name of names.filter(match)) {
    const dir = path.join(tmp, name);
    try {
      if (olderThanMs !== undefined) {
        const { mtimeMs } = await fs.promises.stat(dir);
        if (Date.now() - mtimeMs < olderThanMs) {
          continue;
        }
      }
      // 終了直後は WebView2 の子プロセスがしばらくファイルを掴むため、リトライする
      await fs.promises.rm(dir, { recursive: true, force: true, maxRetries: 40, retryDelay: 250 });
    } catch {
      // 削除できないものは次回の実行で再試行する
    }
  }
}

/**
 * 指定ポートの E2E 用 WebView2 データディレクトリを削除する
 */
export async function removeWebViewData(port: number): Promise<void> {
  await removeTempDirs(
    (name) => name.startsWith('ClipRefiner-') && name.endsWith(`-WebView2-e2e-${port}`),
  );
}

/**
 * 強制終了などで残った E2E 用ディレクトリのうち、古いものを削除する
 */
export async function removeStaleE2eTempDirs(): Promise<void> {
  await removeTempDirs(
    (name) =>
      name.startsWith('clip-refiner-e2e-') ||
      (name.startsWith('ClipRefiner-') && /-WebView2-e2e-\d+$/.test(name)),
    STALE_AGE_MS,
  );
}
