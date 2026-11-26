/**
 * UI ロジックテスト (`tests/ui/`) を実行し、`src/ui/` のカバレッジの下限をファイル単位で強制する
 *
 * Node.js 標準の `--test-coverage-lines` などは全ファイルの合計で判定し、テストが import しない
 * ファイルを集計に含めない。そのため一部のファイルだけ低い状態や、テストの無いファイルの追加を
 * 見逃す。ここではファイルごとに判定し、計測されないファイルは `unmeasuredFiles` に挙げた
 * ものだけを許す
 */
import { globSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { run } from 'node:test';
import { spec } from 'node:test/reporters';
import { fileURLToPath } from 'node:url';

import type { EventData } from 'node:test';

type CoverageSummary = EventData.TestCoverage['summary'];
type FileCoverage = CoverageSummary['files'][number];

const root = join(dirname(fileURLToPath(import.meta.url)), '../..');
const uiRoot = join(root, 'crates/clip-refiner/src/ui');
const testGlob = 'crates/clip-refiner/tests/ui/*.ts';

/** ファイルごとの下限 (%) */
const thresholds = { line: 90, branch: 80, function: 90 };

/**
 * Node.js のテストで計測しないファイル (`src/ui/` からの相対パス)
 *
 * DOM や WebView の IPC を扱い E2E で検証するものと、型宣言だけのものに限る。DOM に依存しない
 * ロジックはここに足さず、`tests/ui/` にテストを書く
 */
const unmeasuredFiles: ReadonlySet<string> = new Set([
    // 型宣言だけで、実行するコードが無い
    'ipc.ts',
    // DOM・WebView の IPC を扱う
    'selector/clip_selector.ts',
    'selector/quick_selector.ts',
    'selector/selector_ipc.ts',
    'settings/dialogs/clip_editor.ts',
    'settings/dialogs/confirm_dialog.ts',
    'settings/events.ts',
    'settings/ipc_apply.ts',
    'settings/main.ts',
    'settings/monitor_ui.ts',
    'settings/panels.ts',
    'settings/regex/regex_feedback.ts',
    'settings/regex/regex_ui.ts',
    'settings/shared.ts',
    'shared/dom.ts',
]);

/**
 * テストを実行し、成否とカバレッジの集計を返す
 */
async function runTests(): Promise<{ success: boolean; coverage: CoverageSummary | null }> {
    let success = false;
    let coverage: CoverageSummary | null = null;

    const stream = run({
        cwd: root,
        globPatterns: [testGlob],
        coverage: true,
        coverageIncludeGlobs: ['crates/clip-refiner/src/ui/**/*.ts'],
    });
    stream.on('test:summary', (data) => {
        // テストファイルごとの集計にも発火する。全体の集計だけが file を持たない
        if (data.file === undefined) {
            success = data.success;
        }
    });
    stream.on('test:coverage', (data) => {
        coverage = data.summary;
    });
    await pipeline(stream, new spec(), process.stdout, { end: false });
    return { success, coverage };
}

/**
 * 行番号の列を、連続する行をまとめた `1-3, 5` の形にする
 */
function formatLines(lines: ReadonlyArray<number>): string {
    const ranges: Array<{ start: number; end: number }> = [];
    for (const line of [...new Set(lines)].sort((a, b) => a - b)) {
        const last = ranges.at(-1);
        if (last && line === last.end + 1) {
            last.end = line;
        } else {
            ranges.push({ start: line, end: line });
        }
    }
    return ranges
        .map(({ start, end }) => (start === end ? `${start}` : `${start}-${end}`))
        .join(', ');
}

/**
 * 1 ファイル分の下限割れを説明する行を返す (下限を満たせば空)
 */
function findShortfalls(path: string, file: FileCoverage): string[] {
    const shortfalls: string[] = [];
    const check = (
        label: string,
        percent: number,
        minimum: number,
        uncovered: ReadonlyArray<{ line: number; count: number }>
    ): void => {
        if (percent >= minimum) {
            return;
        }
        const lines = uncovered.filter(({ count }) => count === 0).map(({ line }) => line);
        shortfalls.push(
            `${path}: ${label} ${percent.toFixed(2)}% (下限 ${minimum}%) 未通過の行: ${formatLines(lines)}`
        );
    };
    check('行', file.coveredLinePercent, thresholds.line, file.lines);
    check('分岐', file.coveredBranchPercent, thresholds.branch, file.branches);
    check('関数', file.coveredFunctionPercent, thresholds.function, file.functions);
    return shortfalls;
}

/**
 * 下限割れ・計測漏れ・古くなった `unmeasuredFiles` を説明する行を返す
 */
function findProblems(coverage: CoverageSummary): string[] {
    const measured = new Map(
        coverage.files.map((file) => [relative(uiRoot, file.path).replaceAll('\\', '/'), file])
    );
    const sources = new Set(
        globSync('**/*.ts', { cwd: uiRoot }).map((path) => path.replaceAll('\\', '/'))
    );

    const problems: string[] = [];
    for (const path of [...sources].sort()) {
        const file = measured.get(path);
        if (unmeasuredFiles.has(path)) {
            if (file) {
                problems.push(`${path}: 計測されているため unmeasuredFiles から外す`);
            }
        } else if (!file) {
            problems.push(`${path}: どのテストからも import されていない`);
        }
        if (file) {
            problems.push(...findShortfalls(path, file));
        }
    }
    for (const path of unmeasuredFiles) {
        if (!sources.has(path)) {
            problems.push(`${path}: 存在しないため unmeasuredFiles から外す`);
        }
    }
    return problems;
}

const { success, coverage } = await runTests();
if (!success) {
    console.error('エラー: UI ロジックテストが失敗しました');
    process.exitCode = 1;
} else if (!coverage) {
    console.error('エラー: カバレッジの集計が得られませんでした');
    process.exitCode = 1;
} else {
    const problems = findProblems(coverage);
    if (problems.length > 0) {
        console.error('エラー: UI のカバレッジが基準を満たしていません:');
        for (const problem of problems) {
            console.error(`  ${problem}`);
        }
        process.exitCode = 1;
    }
}
