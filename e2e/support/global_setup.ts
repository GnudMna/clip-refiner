import { removeStaleE2eTempDirs } from './temp_cleanup.ts';

/**
 * 全テストの開始前に、前回までの実行で残った一時ディレクトリを削除する
 */
export default async function globalSetup(): Promise<void> {
    await removeStaleE2eTempDirs();
}
