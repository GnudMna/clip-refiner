use std::borrow::Cow;
use std::path::Path;

// ======================================================================
// ベースネーム抽出
// ======================================================================
/// パスからベースネーム(ファイル名またはディレクトリ名)を抽出する
///
/// 複数行の入力に対応しており、各行をパスとして処理する
/// 引用符で囲まれたパスも解析可能
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - ベースネームが抽出されたテキスト。変更がない行はそのまま維持される
pub fn extract_basename(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| extract_single_basename(line).map(Cow::Owned))
}

/// パスからベースネームを抽出し、ダブルクォーテーションで囲んで返す
///
/// 複数行の入力に対応しており、抽出結果を引用符("...")で囲む
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - ベースネームが抽出(引用符付き)されたテキスト
pub fn extract_basename_quoted(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        extract_single_basename(line).map(|basename| Cow::Owned(format!("\"{basename}\"")))
    })
}

// ======================================================================
// パス引用符操作
// ======================================================================
/// 行の前後にあるダブルクォーテーションを削除する
///
/// 入力がパスらしい形式であり、かつ引用符で囲まれている場合にのみ削除を行う
/// 複数行の入力に対応している
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 引用符が削除されたテキスト
pub fn remove_path_quotes(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let path_str = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(trimmed);
            if is_path_like_raw(path_str) {
                return Some(Cow::Owned(with_surrounding_whitespace(line, path_str)));
            }
        }
        None
    })
}

/// 行の前後にダブルクォーテーションを付与する
///
/// 入力がパスらしい形式であり、かつ引用符で囲まれていない場合にのみ付与を行う
/// 複数行の入力に対応している
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - 引用符が付与されたテキスト
pub fn add_path_quotes(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !(trimmed.is_empty() || trimmed.starts_with('"') && trimmed.ends_with('"'))
            && is_path_like_raw(trimmed)
        {
            let quoted = format!("\"{trimmed}\"");
            return Some(Cow::Owned(with_surrounding_whitespace(line, &quoted)));
        }
        None
    })
}

// ======================================================================
// パス区切り文字変換
// ======================================================================
/// パス内のバックスラッシュをスラッシュに変換する
///
/// Windows形式のパス区切り文字をUnix/Web形式に変換する
/// 複数行の入力に対応している
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - スラッシュ区切りに変換されたテキスト
pub fn convert_to_forward_slash(text: &str) -> Cow<'_, str> {
    convert_separators(text, '\\', "/")
}

/// パス内のスラッシュをバックスラッシュに変換する
///
/// Unix 形式のパス区切り文字を Windows 形式に変換する
/// 複数行の入力に対応している
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
///
/// # Returns
/// * `Cow<'_, str>` - バックスラッシュ区切りに変換されたテキスト
pub fn convert_to_backslash(text: &str) -> Cow<'_, str> {
    convert_separators(text, '/', "\\")
}

/// パスらしい行の区切り文字を置き換える
///
/// 行頭のインデントと行末の空白は変換せず維持する
///
/// # Arguments
/// * `text` - パスを含む文字列(複数行可)
/// * `from` - 置き換える区切り文字
/// * `to` - 置き換え後の区切り文字
///
/// # Returns
/// * `Cow<'_, str>` - 区切り文字が置き換えられたテキスト。変更がない場合は元のテキスト
fn convert_separators<'a>(text: &'a str, from: char, to: &str) -> Cow<'a, str> {
    super::utils::process_lines(text, |line| {
        let body = line.trim();
        if body.is_empty() || !is_path_like_raw(body) {
            return None;
        }
        let converted = body.replace(from, to);
        if converted == body {
            return None;
        }
        Some(Cow::Owned(with_surrounding_whitespace(line, &converted)))
    })
}

/// 行頭のインデントと行末の空白を維持したまま、本文だけを差し替える
///
/// # Arguments
/// * `line` - 元の1行
/// * `body` - `line` の前後の空白を除いた部分と置き換える文字列
///
/// # Returns
/// * `String` - 元の前後の空白で `body` を挟んだ文字列
fn with_surrounding_whitespace(line: &str, body: &str) -> String {
    let start = line.len() - line.trim_start().len();
    let end = line.trim_end().len();
    format!("{}{body}{}", &line[..start], &line[end..])
}

// ======================================================================
// パス判定ユーティリティ
// ======================================================================
/// 入力がパスらしい形式か判定する(簡易版)
///
/// 先頭が次のいずれかの場合のみパスとみなす。区切り文字を含むだけの文字列は
/// パスとみなさない(URL・`and/or`・`TCP/IP` などの通常の文章を壊さないため)
/// * Windowsドライブレター(例: `C:\`, `D:/`)
/// * `/` または `\` (ルート・UNC `\\server\share` を含む)
/// * `./` `../` `.\` `..\` (相対パス)
///
/// ただし `// comment` のように、区切り文字を 2 つ重ねた直後が空白または行末の場合は
/// UNC ではなくコメントとみなし、パスとみなさない
///
/// # Arguments
/// * `text` - 判定対象の文字列
///
/// # Returns
/// * `bool` - パスらしい場合は `true`、そうでない場合は `false`
fn is_path_like_raw(text: &str) -> bool {
    let b = text.as_bytes();
    let is_sep = |c: u8| c == b'/' || c == b'\\';

    // ルート・UNC。`//` `\\` の直後が空白または行末ならコメントとして除外する
    if b.first().is_some_and(|&c| is_sep(c)) {
        let is_comment =
            b.len() >= 2 && b[1] == b[0] && b.get(2).is_none_or(u8::is_ascii_whitespace);
        return !is_comment;
    }
    // Windowsドライブレター: 先頭が ASCII アルファベット1文字 + ':' + パス区切り文字
    // 例: C:\ や D:/ はパスだが、12:00:00 や key: value などは除外する
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && is_sep(b[2]) {
        return true;
    }
    // 相対パス: ./ ../ (区切りは / と \ のどちらも可)
    let rest = text.strip_prefix("..").or_else(|| text.strip_prefix('.'));
    rest.and_then(|r| r.as_bytes().first())
        .is_some_and(|&c| is_sep(c))
}

/// 1つの行からベースネームを抽出する
///
/// # Arguments
/// * `line` - 処理対象の1行
///
/// # Returns
/// * `Option<String>` - ベースネームが抽出できた場合は `Some(ベースネーム)`、パスでない場合や抽出できない場合は `None`
fn extract_single_basename(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let path_str = if trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(trimmed)
    } else {
        trimmed
    };

    if !is_path_like_raw(path_str) {
        return None;
    }

    Path::new(path_str)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 複数行からのベースネーム抽出できるようにすること
    #[test]
    fn test_extract_basename_multiline() {
        let input = "\"C:\\Program Files (x86)\"\n\"C:\\ProgramData\"\n\"C:\\Program Files\"";
        let expected = "Program Files (x86)\nProgramData\nProgram Files";
        assert_eq!(extract_basename(input), expected);
    }

    /// パスと非パスが混在する場合のベースネーム抽出できるようにすること
    #[test]
    fn test_extract_basename_mixed() {
        let input = "C:\\foo\\bar.txt\nNot a path\n/tmp/test.log";
        let expected = "bar.txt\nNot a path\ntest.log";
        assert_eq!(extract_basename(input), expected);
    }

    /// パスの引用符削除できるようにすること
    #[test]
    fn test_remove_path_quotes() {
        let input = "\"C:\\foo\\bar.txt\"\n\"Not a path\"\nD:\\data";
        let expected = "C:\\foo\\bar.txt\n\"Not a path\"\nD:\\data";
        assert_eq!(remove_path_quotes(input), expected);
    }

    /// パスへの引用符付与できるようにすること
    #[test]
    fn test_add_path_quotes() {
        let input = "C:\\foo\\bar.txt\n\"Already quoted\"\nE:\\work";
        let expected = "\"C:\\foo\\bar.txt\"\n\"Already quoted\"\n\"E:\\work\"";
        assert_eq!(add_path_quotes(input), expected);
    }

    /// 引用符の付与で行頭のインデントと行末の空白を維持すること
    #[test]
    fn test_add_path_quotes_preserves_whitespace() {
        let input = "  C:\\foo\t\n\t/tmp/x";
        let expected = "  \"C:\\foo\"\t\n\t\"/tmp/x\"";
        assert_eq!(add_path_quotes(input), expected);
    }

    /// 引用符の除去で行頭のインデントと行末の空白を維持すること
    #[test]
    fn test_remove_path_quotes_preserves_whitespace() {
        let input = "  \"C:\\foo\"\t\n\t\"/tmp/x\"";
        let expected = "  C:\\foo\t\n\t/tmp/x";
        assert_eq!(remove_path_quotes(input), expected);
    }

    /// CRLF と LF が混在していても行ごとに引用符を付与すること
    #[test]
    fn test_add_path_quotes_mixed_line_endings() {
        let input = "C:\\a\r\nfoo\nC:\\b";
        let expected = "\"C:\\a\"\r\nfoo\n\"C:\\b\"";
        assert_eq!(add_path_quotes(input), expected);
    }

    /// スラッシュ区切りへの変換できるようにすること
    #[test]
    fn test_convert_to_forward_slash() {
        let input = "C:\\Users\\Test\\file.txt";
        let expected = "C:/Users/Test/file.txt";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// 複数行のスラッシュ区切り変換できるようにすること
    #[test]
    fn test_convert_to_forward_slash_multiline() {
        let input = "C:\\foo\\bar.txt\nD:\\data\\test.log";
        let expected = "C:/foo/bar.txt\nD:/data/test.log";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// 既にスラッシュ区切りの場合は変更しないようにすること
    #[test]
    fn test_convert_to_forward_slash_already_slash() {
        let input = "/usr/local/bin";
        let result = convert_to_forward_slash(input);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, input);
    }

    /// バックスラッシュ区切りへの変換できるようにすること
    #[test]
    fn test_convert_to_backslash() {
        let input = "/usr/local/bin";
        let expected = "\\usr\\local\\bin";
        assert_eq!(convert_to_backslash(input), expected);
    }

    /// 複数行のバックスラッシュ区切り変換できるようにすること
    #[test]
    fn test_convert_to_backslash_multiline() {
        let input = "/home/user/file.txt\n/tmp/test.log";
        let expected = "\\home\\user\\file.txt\n\\tmp\\test.log";
        assert_eq!(convert_to_backslash(input), expected);
    }

    /// 既にバックスラッシュ区切りの場合は変更しないようにすること
    #[test]
    fn test_convert_to_backslash_already_backslash() {
        let input = "C:\\Windows\\System32";
        let result = convert_to_backslash(input);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, input);
    }

    /// 異なる種類のパスが混在する場合の変換できるようにすること
    #[test]
    fn test_convert_mixed_content() {
        let input = "C:\\foo\\bar.txt\nNot a path\nD:\\data";
        let expected = "C:/foo/bar.txt\nNot a path\nD:/data";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// スペースを含むパスからのベースネーム抽出できるようにすること
    #[test]
    fn test_extract_basename_spaces() {
        let input = "C:\\Program Files\\My App\\app.exe";
        assert_eq!(extract_basename(input), "app.exe");
    }

    /// 相対パスからのベースネーム抽出できるようにすること
    #[test]
    fn test_extract_basename_relative() {
        let input = "./foo/bar/baz.txt";
        assert_eq!(extract_basename(input), "baz.txt");
    }

    /// URL を各パス変換で変更しないようにすること
    #[test]
    fn test_url_is_not_converted() {
        let input = "https://example.com/a/b?x=1";
        assert!(matches!(convert_to_backslash(input), Cow::Borrowed(_)));
        assert!(matches!(convert_to_forward_slash(input), Cow::Borrowed(_)));
        assert!(matches!(add_path_quotes(input), Cow::Borrowed(_)));
        assert!(matches!(extract_basename(input), Cow::Borrowed(_)));
    }

    /// 区切り文字を含む通常の文章を各パス変換で変更しないようにすること
    #[test]
    fn test_prose_with_slash_is_not_converted() {
        for input in ["TCP/IP and/or UDP", "and/or", "foo/bar.txt", "a\\b"] {
            assert!(
                matches!(convert_to_backslash(input), Cow::Borrowed(_)),
                "{input}"
            );
            assert!(
                matches!(convert_to_forward_slash(input), Cow::Borrowed(_)),
                "{input}"
            );
            assert!(
                matches!(add_path_quotes(input), Cow::Borrowed(_)),
                "{input}"
            );
        }
    }

    /// 行頭のインデントと行末の空白を維持して変換すること
    #[test]
    fn test_convert_preserves_surrounding_whitespace() {
        assert_eq!(
            convert_to_backslash("  /usr/bin\n\t/tmp/x  \r\n"),
            "  \\usr\\bin\n\t\\tmp\\x  \r\n"
        );
        assert_eq!(
            convert_to_forward_slash("    C:\\a\\b\n  \\\\server\\share"),
            "    C:/a/b\n  //server/share"
        );
        // 変更がない場合はインデントがあっても Borrowed を返す
        assert!(matches!(
            convert_to_backslash("  C:\\a\\b"),
            Cow::Borrowed(_)
        ));
    }

    /// `//` や `\\` の直後が空白のコメント風の行は変換しないこと
    #[test]
    fn test_comment_like_line_is_not_converted() {
        for input in ["// comment here", "  // comment", "//", "\\\\ comment"] {
            assert!(
                matches!(convert_to_backslash(input), Cow::Borrowed(_)),
                "{input}"
            );
            assert!(
                matches!(convert_to_forward_slash(input), Cow::Borrowed(_)),
                "{input}"
            );
            assert!(
                matches!(add_path_quotes(input), Cow::Borrowed(_)),
                "{input}"
            );
        }
        // 直後が空白でない `//server/share` は UNC として変換する
        assert_eq!(convert_to_backslash("//server/share"), "\\\\server\\share");
    }

    /// ルート・UNC・相対パスをパスとして変換すること
    #[test]
    fn test_root_unc_and_relative_paths_are_converted() {
        assert_eq!(convert_to_backslash("/usr/bin"), "\\usr\\bin");
        assert_eq!(convert_to_backslash("./a/b"), ".\\a\\b");
        assert_eq!(convert_to_backslash("../a/b"), "..\\a\\b");
        assert_eq!(
            convert_to_forward_slash("\\\\server\\share\\f"),
            "//server/share/f"
        );
        assert_eq!(convert_to_forward_slash(".\\a\\b"), "./a/b");
        assert_eq!(
            add_path_quotes("\\\\server\\share"),
            "\"\\\\server\\share\""
        );
    }
}
