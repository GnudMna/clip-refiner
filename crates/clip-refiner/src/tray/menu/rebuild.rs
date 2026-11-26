//! 有効トグル付き動的レコード・サブメニューの再構築

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, IsMenuItem, MenuId, MenuItem, PredefinedMenuItem, Submenu};

/// トレイの動的レコード一覧メニュー用スニペット最大文字数
pub(crate) const DYNAMIC_RECORD_MENU_SNIPPET_MAX_CHARS: usize = 30;

// ======================================================================
// パブリック関数
// ======================================================================
/// メニュー項目の表示文字列として安全な形に変換する
///
/// muda は `&` をショートカット文字の指定として解釈するため `&&` にする
/// 改行・タブは表示が崩れるため空白に置き換える
///
/// # Arguments
/// * `text` - メニューに表示するユーザー由来の文字列
///
/// # Returns
/// * `String` - エスケープ済みの表示文字列
pub(crate) fn escape_menu_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&&"),
            '\r' | '\n' | '\t' => escaped.push(' '),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// 有効トグル + 区切り + 動的レコード (+ 区切り) + フッターでサブメニューを再構築する
///
/// 既存項目をすべて削除し、`enabled_item`・レコード・`footer_items` の順に載せ直す
/// レコードが 1 件以上ある場合のみ、レコード群とフッターの間に区切りを入れる
///
/// # Arguments
/// * `submenu` - 再構築対象のサブメニュー
/// * `records` - 項目 ID とストア内インデックスの対応表 (呼び出し側で保持)
/// * `enabled_item` - 先頭に置く有効/無効トグル
/// * `entry_labels` - `(ストア内インデックス, 表示ラベル)` の列
/// * `footer_items` - 末尾に置く固定項目 (クリア・取り消しなど)
///
/// # Returns
/// * `Result<()>` - 成功時は `Ok(())`
pub(crate) fn rebuild_toggle_records_submenu(
    submenu: &Submenu,
    records: &mut Vec<(MenuId, usize)>,
    enabled_item: &CheckMenuItem,
    entry_labels: impl IntoIterator<Item = (usize, String)>,
    footer_items: &[&dyn IsMenuItem],
) -> Result<()> {
    records.clear();

    for _ in 0..submenu.items().len() {
        submenu.remove_at(0);
    }

    submenu.append_items(&[
        enabled_item as &dyn IsMenuItem,
        &PredefinedMenuItem::separator() as &dyn IsMenuItem,
    ])?;

    let mut has_records = false;
    for (index, label) in entry_labels {
        let item = MenuItem::new(escape_menu_text(&label), true, None);
        records.push((item.id().clone(), index));
        submenu.append_items(&[&item as &dyn IsMenuItem])?;
        has_records = true;
    }

    if has_records {
        submenu.append_items(&[&PredefinedMenuItem::separator() as &dyn IsMenuItem])?;
    }

    if !footer_items.is_empty() {
        submenu.append_items(footer_items)?;
    }

    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `&` は `&&` にし、改行とタブは空白に置き換えるようにすること
    #[test]
    fn escape_menu_text_escapes_ampersand_and_control_chars() {
        assert_eq!(escape_menu_text("Q&A"), "Q&&A");
        assert_eq!(escape_menu_text("?a=1&b=2"), "?a=1&&b=2");
        assert_eq!(escape_menu_text("a\r\nb\tc\nd"), "a  b c d");
        assert_eq!(escape_menu_text("plain"), "plain");
    }

    /// 動的レコードのラベルがエスケープされて項目に載るようにすること
    #[test]
    fn rebuild_escapes_record_labels() {
        let enabled = CheckMenuItem::new("有効", true, true, None);
        let submenu = Submenu::new("一覧", true);
        let mut records = Vec::new();

        rebuild_toggle_records_submenu(
            &submenu,
            &mut records,
            &enabled,
            [(0, "Q&A\nnext".to_string())],
            &[],
        )
        .expect("再構築に失敗");

        // 有効 + 区切り + レコード + 区切り
        let item = submenu.items()[2]
            .as_menuitem()
            .map(MenuItem::text)
            .expect("レコード項目が MenuItem ではない");
        assert_eq!(item, "Q&&A next");
    }

    /// レコードありのとき有効トグル・レコード・フッターの順になるようにすること
    #[test]
    fn rebuild_with_records_keeps_enabled_records_footer_order() {
        let enabled = CheckMenuItem::new("有効", true, true, None);
        let clear = MenuItem::new("クリア", true, None);
        let submenu = Submenu::new("一覧", true);
        let mut records = Vec::new();

        rebuild_toggle_records_submenu(
            &submenu,
            &mut records,
            &enabled,
            [(0, "first".into()), (1, "second".into())],
            &[&clear as &dyn IsMenuItem],
        )
        .expect("再構築に失敗");

        // 有効 + 区切り + 2 レコード + 区切り + クリア
        assert_eq!(submenu.items().len(), 6);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].1, 0);
        assert_eq!(records[1].1, 1);
    }

    /// レコードなしのとき有効トグルとフッターのみになるようにすること
    #[test]
    fn rebuild_without_records_omits_record_separator() {
        let enabled = CheckMenuItem::new("有効", true, false, None);
        let undo = MenuItem::new("取り消し", true, None);
        let clear = MenuItem::new("クリア", true, None);
        let submenu = Submenu::new("一覧", true);
        let mut records = vec![(MenuItem::new("stale", true, None).id().clone(), 0)];

        rebuild_toggle_records_submenu(
            &submenu,
            &mut records,
            &enabled,
            std::iter::empty(),
            &[&undo as &dyn IsMenuItem, &clear as &dyn IsMenuItem],
        )
        .expect("再構築に失敗");

        // 有効 + 区切り + 取り消し + クリア (レコード用区切りなし)
        assert_eq!(submenu.items().len(), 4);
        assert_eq!(records.len(), 0);
    }
}
