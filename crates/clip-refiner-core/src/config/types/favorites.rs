// ======================================================================
// お気に入り変換モード
// ======================================================================
/// お気に入り変換モードの切り替え結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteToggleResult {
    /// お気に入りへ追加した
    Added,
    /// お気に入りから削除した
    Removed,
    /// 登録件数の上限に達している
    LimitReached,
}

/// お気に入り変換モードの移動方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteMoveDirection {
    /// 上へ移動
    Up,
    /// 下へ移動
    Down,
}
