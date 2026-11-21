/**
 * markdownlint のカスタムルール: 表の区切り行を `| :--- | :--- |` の形にそろえる
 *
 * 列ごとの区切りの書き方 (`---` / `:---` / `:-----:` など) は標準ルールで統一できないため、
 * `.claude/rules/docs.md` の規約をここで検証する。セルの空白の埋め方は MD060 (`compact`) が担う
 */

const delimiterRow = /^\|(?: :--- \|)+$/;

/**
 * micromark のトークン木から指定した種類のトークンを集める
 *
 * @param {ReadonlyArray<import('markdownlint').MicromarkToken>} tokens
 * @param {string} type
 * @returns {import('markdownlint').MicromarkToken[]}
 */
function collectTokens(tokens, type) {
    return tokens.flatMap((token) => [
        ...(token.type === type ? [token] : []),
        ...collectTokens(token.children, type),
    ]);
}

/** @type {import('markdownlint').Rule} */
const tableDelimiter = {
    names: ['table-delimiter-left-aligned'],
    description: 'Table delimiter row should be "| :--- | :--- |"',
    tags: ['table'],
    parser: 'micromark',
    function: (params, onError) => {
        for (const row of collectTokens(params.parsers.micromark.tokens, 'tableDelimiterRow')) {
            const line = params.lines[row.startLine - 1];
            // 箇条書きの中の表は字下げされる
            if (!delimiterRow.test(line.trim())) {
                onError({ lineNumber: row.startLine, context: line });
            }
        }
    },
};

export default tableDelimiter;
