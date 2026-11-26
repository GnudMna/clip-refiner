/**
 * markdownlint のカスタムルール: 表の区切り行を `| :--- | :--- |` の形にそろえる
 *
 * 列ごとの区切りの書き方 (`---` / `:---` / `:-----:` など) は標準ルールで統一できないため、
 * `.claude/rules/docs.md` の規約をここで検証する。セルの空白の埋め方は MD060 (`compact`) が担う
 */

import type { MicromarkToken, Rule } from 'markdownlint';

const delimiterRow = /^\|(?: :--- \|)+$/;

/**
 * micromark のトークン木から指定した種類のトークンを集める
 */
function collectTokens(tokens: ReadonlyArray<MicromarkToken>, type: string): MicromarkToken[] {
    return tokens.flatMap((token) => [
        ...(token.type === type ? [token] : []),
        ...collectTokens(token.children, type),
    ]);
}

const tableDelimiter: Rule = {
    names: ['table-delimiter-left-aligned'],
    description: 'Table delimiter row should be "| :--- | :--- |"',
    tags: ['table'],
    parser: 'micromark',
    function: (params, onError) => {
        for (const row of collectTokens(params.parsers.micromark.tokens, 'tableDelimiterRow')) {
            const line = params.lines[row.startLine - 1] ?? '';
            // 箇条書きの中の表は字下げされる
            if (!delimiterRow.test(line.trim())) {
                onError({ lineNumber: row.startLine, context: line });
            }
        }
    },
};

export default tableDelimiter;
