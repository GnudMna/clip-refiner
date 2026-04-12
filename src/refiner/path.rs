use std::borrow::Cow;
use std::path::Path;

/// パスからベースネーム（ファイル名またはディレクトリ名）を抽出する
///
/// 複数行の入力に対応しており、各行をパスとして処理します。
/// 引用符で囲まれたパスも解析可能です。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - ベースネームが抽出されたテキスト。変更がない行はそのまま維持されます。
pub fn extract_basename(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| extract_single_basename(line).map(Cow::Owned))
}

/// パスからベースネームを抽出し、ダブルクォーテーションで囲んで返す
///
/// 複数行の入力に対応しており、抽出結果を引用符（"..."）で囲みます。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - ベースネームが抽出（引用符付き）されたテキスト。
pub fn extract_basename_quoted(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        extract_single_basename(line).map(|basename| Cow::Owned(format!("\"{}\"", basename)))
    })
}

/// 行の前後にあるダブルクォーテーションを削除する
///
/// 入力がパスらしい形式であり、かつ引用符で囲まれている場合にのみ削除を行います。
/// 複数行の入力に対応しています。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - 引用符が削除されたテキスト。
pub fn remove_path_quotes(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let path_str = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(trimmed);
            if is_path_like_raw(path_str) {
                return Some(Cow::Borrowed(path_str));
            }
        }
        None
    })
}

/// 行の前後にダブルクォーテーションを付与する
///
/// 入力がパスらしい形式であり、かつ引用符で囲まれていない場合にのみ付与を行います。
/// 複数行の入力に対応しています。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - 引用符が付与されたテキスト。
pub fn add_path_quotes(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !(trimmed.is_empty() || trimmed.starts_with('"') && trimmed.ends_with('"'))
            && is_path_like_raw(trimmed)
        {
            return Some(Cow::Owned(format!("\"{}\"", trimmed)));
        }
        None
    })
}

/// パス内のバックスラッシュをスラッシュに変換する
///
/// Windows形式のパス区切り文字をUnix/Web形式に変換します。
/// 複数行の入力に対応しています。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - スラッシュ区切りに変換されたテキスト。
pub fn convert_to_forward_slash(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() && is_path_like_raw(trimmed) {
            let converted = trimmed.replace('\\', "/");
            if converted != trimmed {
                return Some(Cow::Owned(converted));
            }
        }
        None
    })
}

/// パス内のスラッシュをバックスラッシュに変換する
///
/// Unix形式のパス区切り文字をWindows形式に変換します。
/// 複数行の入力に対応しています。
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Cow<'_, str>` - バックスラッシュ区切りに変換されたテキスト。
pub fn convert_to_backslash(text: &str) -> Cow<'_, str> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() && is_path_like_raw(trimmed) {
            let converted = trimmed.replace('/', "\\");
            if converted != trimmed {
                return Some(Cow::Owned(converted));
            }
        }
        None
    })
}

/// 入力がパスらしい形式か判定する（簡易版）
///
/// スラッシュ、バックスラッシュ、またはWindowsドライブレター形式（例: `C:\`, `D:/`）が
/// 含まれているかを確認します。単独のコロン（時刻文字列・URLポート番号・YAMLキーなど）は
/// パスとみなしません。
///
/// # Arguments
/// * `text` - 判定対象の文字列
///
/// # Returns
/// * `bool` - パスらしい場合は `true`、そうでない場合は `false`。
fn is_path_like_raw(text: &str) -> bool {
    if text.contains('/') || text.contains('\\') {
        return true;
    }
    // Windowsドライブレター: 先頭が ASCII アルファベット1文字 + ':' + パス区切り文字
    // 例: C:\ や D:/ はパスだが、12:00:00 や key: value などは除外する
    let b = text.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// 1つの行からベースネームを抽出する
///
/// # Arguments
/// * `line` - 処理対象の1行
///
/// # Returns
/// * `Option<String>` - ベースネームが抽出できた場合は `Some(ベースネーム)` を返す
fn extract_single_basename(line: &str) -> Option<String> {
    let trimmed = line.trim();
    // 引用符があれば外す
    let path_str = if trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(trimmed)
    } else {
        trimmed
    };

    // パスらしいかチェック
    if !is_path_like_raw(path_str) {
        return None;
    }

    Path::new(path_str)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 複数行からのベースネーム抽出テスト
    #[test]
    fn test_extract_basename_multiline() {
        let input = "\"C:\\Program Files (x86)\"\n\"C:\\ProgramData\"\n\"C:\\Program Files\"";
        let expected = "Program Files (x86)\nProgramData\nProgram Files";
        assert_eq!(extract_basename(input), expected);
    }

    /// パスと非パスが混在する場合のベースネーム抽出テスト
    #[test]
    fn test_extract_basename_mixed() {
        let input = "C:\\foo\\bar.txt\nNot a path\n/tmp/test.log";
        let expected = "bar.txt\nNot a path\ntest.log";
        assert_eq!(extract_basename(input), expected);
    }

    /// パスの引用符削除テスト
    #[test]
    fn test_remove_path_quotes() {
        let input = "\"C:\\foo\\bar.txt\"\n\"Not a path\"\nD:\\data";
        let expected = "C:\\foo\\bar.txt\n\"Not a path\"\nD:\\data";
        assert_eq!(remove_path_quotes(input), expected);
    }

    /// パスへの引用符付与テスト
    #[test]
    fn test_add_path_quotes() {
        let input = "C:\\foo\\bar.txt\n\"Already quoted\"\nE:\\work";
        let expected = "\"C:\\foo\\bar.txt\"\n\"Already quoted\"\n\"E:\\work\"";
        assert_eq!(add_path_quotes(input), expected);
    }

    /// スラッシュ区切りへの変換テスト
    #[test]
    fn test_convert_to_forward_slash() {
        let input = "C:\\Users\\Test\\file.txt";
        let expected = "C:/Users/Test/file.txt";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// 複数行のスラッシュ区切り変換テスト
    #[test]
    fn test_convert_to_forward_slash_multiline() {
        let input = "C:\\foo\\bar.txt\nD:\\data\\test.log";
        let expected = "C:/foo/bar.txt\nD:/data/test.log";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// 既にスラッシュ区切りの場合に変更されないことを確認するテスト
    #[test]
    fn test_convert_to_forward_slash_already_slash() {
        let input = "/usr/local/bin";
        let result = convert_to_forward_slash(input);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, input);
    }

    /// バックスラッシュ区切りへの変換テスト
    #[test]
    fn test_convert_to_backslash() {
        let input = "/usr/local/bin";
        let expected = "\\usr\\local\\bin";
        assert_eq!(convert_to_backslash(input), expected);
    }

    /// 複数行のバックスラッシュ区切り変換テスト
    #[test]
    fn test_convert_to_backslash_multiline() {
        let input = "/home/user/file.txt\n/tmp/test.log";
        let expected = "\\home\\user\\file.txt\n\\tmp\\test.log";
        assert_eq!(convert_to_backslash(input), expected);
    }

    /// 既にバックスラッシュ区切りの場合に変更されないことを確認するテスト
    #[test]
    fn test_convert_to_backslash_already_backslash() {
        let input = "C:\\Windows\\System32";
        let result = convert_to_backslash(input);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, input);
    }

    /// 異なる種類のパスが混在する場合の変換テスト
    #[test]
    fn test_convert_mixed_content() {
        let input = "C:\\foo\\bar.txt\nNot a path\nD:\\data";
        let expected = "C:/foo/bar.txt\nNot a path\nD:/data";
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    /// スペースを含むパスからのベースネーム抽出テスト
    #[test]
    fn test_extract_basename_spaces() {
        let input = "C:\\Program Files\\My App\\app.exe";
        assert_eq!(extract_basename(input), "app.exe");
    }

    /// 相対パスからのベースネーム抽出テスト
    #[test]
    fn test_extract_basename_relative() {
        let input = "./foo/bar/baz.txt";
        assert_eq!(extract_basename(input), "baz.txt");
    }
}
