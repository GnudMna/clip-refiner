export interface QuickSelectorMode {
    id: string;
    label: string;
    category: string;
    value?: string;
    favorite?: boolean;
    favoriteIndex?: number;
    /** 画像を出力するモードか */
    producesImage?: boolean;
    /** `header` はカテゴリ見出し行 */
    listSection?: 'favorites' | 'category' | 'header';
    /** 見出し行のカテゴリが展開中かどうか */
    expanded?: boolean;
    /** 見出し行のカテゴリに属するモード数 */
    count?: number;
}

/**
 * クイックセレクターの DOM 非依存ロジック
 */
const QuickSelectorLogic = (function () {
    'use strict';

    /** お気に入りセクションの表示名 */
    const FAVORITES_CATEGORY: string = 'お気に入り';

    /** カテゴリ見出し行の ID 接頭辞 (モード ID と衝突しない) */
    const CATEGORY_ROW_PREFIX: string = 'category:';

    /**
     * ID からモードを取得する
     */
    function modeById(modes: QuickSelectorMode[], id: string): QuickSelectorMode | undefined {
        return modes.find((mode) => mode.id === id);
    }

    /**
     * お気に入りモード ID を表示順で返す
     */
    function favoriteIds(modes: QuickSelectorMode[]): string[] {
        return modes
            .filter((mode) => mode.favorite)
            .sort((a, b) => {
                const left = a.favoriteIndex ?? Number.MAX_SAFE_INTEGER;
                const right = b.favoriteIndex ?? Number.MAX_SAFE_INTEGER;
                return left - right;
            })
            .map((mode) => mode.id);
    }

    /**
     * 検索クエリにモードが一致するかどうか
     *
     * @param query - 正規化済みクエリ
     */
    function modeMatches(mode: QuickSelectorMode, query: string): boolean {
        if (!query) {
            return true;
        }

        const fields = [mode.label, mode.category, mode.value, mode.id];
        return fields.some((field) => field && field.toLowerCase().includes(query));
    }

    /**
     * カテゴリ見出し行の ID を返す
     */
    function categoryRowId(category: string): string {
        return CATEGORY_ROW_PREFIX + category;
    }

    /**
     * 行がカテゴリ見出しかどうか
     */
    function isCategoryRow(row: QuickSelectorMode | undefined | null): boolean {
        return !!row && row.listSection === 'header';
    }

    /**
     * 検索クエリに応じた表示用の行一覧を組み立てる
     *
     * 未検索時はお気に入り・カテゴリなしのモード・カテゴリ見出しの順に並べ、
     * 展開中のカテゴリだけ見出しの直後にモードを並べる。検索中はカテゴリを無視して一致したモードだけを返す
     *
     * @param query - 正規化済みクエリ
     * @param expandedCategories - 展開中のカテゴリ名
     */
    function buildDisplayList(
        modes: QuickSelectorMode[],
        query: string,
        expandedCategories: Set<string>
    ): QuickSelectorMode[] {
        if (query) {
            return modes.filter((mode) => modeMatches(mode, query));
        }

        const display: QuickSelectorMode[] = [];
        for (const id of favoriteIds(modes)) {
            const mode = modeById(modes, id);
            if (mode) {
                display.push({ ...mode, listSection: 'favorites' });
            }
        }

        const byCategory: Map<string, QuickSelectorMode[]> = new Map();
        for (const mode of modes) {
            if (!mode.category) {
                display.push({ ...mode, listSection: 'category' });
                continue;
            }
            let members = byCategory.get(mode.category);
            if (!members) {
                members = [];
                byCategory.set(mode.category, members);
            }
            members.push(mode);
        }

        for (const [category, members] of byCategory) {
            const expanded = expandedCategories.has(category);
            display.push({
                id: categoryRowId(category),
                label: category,
                category,
                listSection: 'header',
                expanded,
                count: members.length,
            });
            if (expanded) {
                for (const mode of members) {
                    display.push({ ...mode, listSection: 'category' });
                }
            }
        }
        return display;
    }

    /**
     * 指定行が属するカテゴリ見出し行のインデックスを返す
     *
     * @returns 見出しに属さない行 (お気に入り・カテゴリなし・検索結果) は `-1`
     */
    function categoryRowIndexOf(rows: QuickSelectorMode[], index: number): number {
        const row = rows[index];
        if (!row || !row.category || row.listSection === 'favorites') {
            return -1;
        }
        if (isCategoryRow(row)) {
            return index;
        }
        if (row.listSection !== 'category') {
            return -1;
        }
        for (let i = index - 1; i >= 0; i--) {
            const candidate = rows[i];
            if (candidate && isCategoryRow(candidate)) {
                return candidate.category === row.category ? i : -1;
            }
        }
        return -1;
    }

    /**
     * 開閉状態を切り替えた後の展開カテゴリ集合を返す
     */
    function withCategoryExpanded(
        expandedCategories: Set<string>,
        category: string,
        expand: boolean
    ): Set<string> {
        const next = new Set(expandedCategories);
        if (expand) {
            next.add(category);
        } else {
            next.delete(category);
        }
        return next;
    }

    /**
     * アプリ側から渡された展開カテゴリを集合へ変換する
     *
     * @returns 配列でない値や文字列以外の要素は無視する
     */
    function toExpandedCategories(categories: unknown): Set<string> {
        return new Set(
            Array.isArray(categories) ? categories.filter((value) => typeof value === 'string') : []
        );
    }

    /**
     * 展開中カテゴリの通知 IPC メッセージを組み立てる
     */
    function expandedCategoriesMessage(expandedCategories: Set<string>): string {
        return 'expanded-categories:' + JSON.stringify([...expandedCategories]);
    }

    /**
     * カテゴリ見出し用のラベルを返す
     */
    function sectionCategory(mode: QuickSelectorMode, query: string): string | null {
        if (query) {
            return null;
        }
        if (mode.listSection === 'favorites') {
            return FAVORITES_CATEGORY;
        }
        if (mode.listSection === 'header') {
            return mode.category;
        }
        return mode.category || '';
    }

    /**
     * フィルタ後に維持する選択インデックスを求める
     *
     * お気に入りは「お気に入り」節とカテゴリ節に同じ id で並ぶため、id と listSection の両方で照合する
     * 同じ節に見つからないときは id だけで探し、それも無ければ先頭にする
     *
     * @param previous - 再描画前の選択行
     */
    function nextIndexAfterFilter(
        filtered: QuickSelectorMode[],
        previous: QuickSelectorMode | undefined | null
    ): number {
        if (!previous || !previous.id) {
            return 0;
        }
        const sameSection = filtered.findIndex(
            (mode) => mode.id === previous.id && mode.listSection === previous.listSection
        );
        if (sameSection !== -1) {
            return sameSection;
        }
        const sameId = filtered.findIndex((mode) => mode.id === previous.id);
        return sameId !== -1 ? sameId : 0;
    }

    /**
     * お気に入り並べ替えボタンを表示するかどうか
     */
    function shouldShowReorderButtons(mode: QuickSelectorMode, query: string): boolean {
        return !query && mode.listSection === 'favorites' && !!mode.favorite;
    }

    /**
     * 選択中のお気に入りを上下移動できるかどうか
     *
     * @param query - 正規化済みクエリ
     */
    function canMoveSelectedFavorite(query: string, mode: QuickSelectorMode | undefined): boolean {
        if (query) {
            return false;
        }
        return !!(mode && mode.listSection === 'favorites' && mode.favorite);
    }

    /**
     * モード行の内部 HTML を組み立てる
     */
    function buildItemInnerHtml(
        mode: QuickSelectorMode,
        query: string,
        isCurrent: boolean,
        highlightText: (text: string, query: string) => string
    ): string {
        return (
            '<div class="mode-info">' +
            '<span class="label">' +
            highlightText(mode.label, query) +
            '</span>' +
            (isCurrent ? '<span class="current-badge">自動変換中</span>' : '') +
            '</div>' +
            (query && mode.value
                ? '<span class="value-hint">' + highlightText(mode.value, query) + '</span>'
                : '')
        );
    }

    /**
     * 現在のクリップボードへ一回だけ適用する IPC メッセージを組み立てる
     */
    function applyMessage(id: string): string {
        return 'apply:' + id;
    }

    /**
     * 自動変換モードに設定する IPC メッセージを組み立てる
     */
    function selectMessage(id: string): string {
        return 'select:' + id;
    }

    /**
     * お気に入りトグル IPC メッセージを組み立てる
     */
    function toggleFavoriteMessage(id: string): string {
        return 'toggle-favorite:' + id;
    }

    /**
     * お気に入り並べ替え IPC メッセージを組み立てる
     */
    function moveFavoriteMessage(id: string, direction: 'up' | 'down'): string {
        return 'move-favorite:' + id + ':' + direction;
    }

    return {
        FAVORITES_CATEGORY,
        categoryRowId,
        isCategoryRow,
        categoryRowIndexOf,
        withCategoryExpanded,
        toExpandedCategories,
        expandedCategoriesMessage,
        modeById,
        favoriteIds,
        modeMatches,
        buildDisplayList,
        sectionCategory,
        nextIndexAfterFilter,
        shouldShowReorderButtons,
        canMoveSelectedFavorite,
        buildItemInnerHtml,
        applyMessage,
        selectMessage,
        toggleFavoriteMessage,
        moveFavoriteMessage,
    };
})();

export default QuickSelectorLogic;
