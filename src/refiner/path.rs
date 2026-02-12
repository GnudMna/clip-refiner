use std::path::Path;

/// パスからベースネームを抽出する
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行でベースネームが抽出できた場合は `Some(加工後テキスト)` を返す
pub fn extract_basename(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        extract_single_basename(line).map(|basename| (basename, true))
    })
}

/// パスからベースネームを抽出し、ダブルクォーテーションで囲んで返す
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行でベースネームが抽出できた場合は `Some(加工後テキスト)` を返す
pub fn extract_basename_quoted(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        extract_single_basename(line).map(|basename| (format!("\"{}\"", basename), true))
    })
}

/// パスの前後にあるダブルクォーテーションを削除する
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行で引用符が削除できた場合は `Some(加工後テキスト)` を返す
pub fn remove_path_quotes(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let path_str = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(trimmed);
            if is_path_like_raw(path_str) {
                return Some((path_str.to_string(), true));
            }
        }
        None
    })
}

/// パスの前後にダブルクォーテーションを付与する
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行で引用符が付与できた場合は `Some(加工後テキスト)` を返す
pub fn add_path_quotes(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !(trimmed.starts_with('"') && trimmed.ends_with('"')) {
            if is_path_like_raw(trimmed) {
                return Some((format!("\"{}\"", trimmed), true));
            }
        }
        None
    })
}

/// パスのバックスラッシュをスラッシュに変換する
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行で変換できた場合は `Some(加工後テキスト)` を返す
pub fn convert_to_forward_slash(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() && is_path_like_raw(trimmed) {
            let converted = trimmed.replace('\\', "/");
            if converted != trimmed {
                return Some((converted, true));
            }
        }
        None
    })
}

/// パスのスラッシュをバックスラッシュに変換する
///
/// # Arguments
/// * `text` - パスを含む文字列（複数行可）
///
/// # Returns
/// * `Option<String>` - 少なくとも1行で変換できた場合は `Some(加工後テキスト)` を返す
pub fn convert_to_backslash(text: &str) -> Option<String> {
    super::utils::process_lines(text, |line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() && is_path_like_raw(trimmed) {
            let converted = trimmed.replace('/', "\\");
            if converted != trimmed {
                return Some((converted, true));
            }
        }
        None
    })
}

fn is_path_like_raw(text: &str) -> bool {
    text.contains('/') || text.contains('\\') || text.contains(':')
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

    #[test]
    fn test_extract_basename_multiline() {
        let input = "\"C:\\Program Files (x86)\"\n\"C:\\ProgramData\"\n\"C:\\Program Files\"";
        let expected = Some("Program Files (x86)\nProgramData\nProgram Files".to_string());
        assert_eq!(extract_basename(input), expected);
    }

    #[test]
    fn test_extract_basename_mixed() {
        let input = "C:\\foo\\bar.txt\nNot a path\n/tmp/test.log";
        let expected = Some("bar.txt\nNot a path\ntest.log".to_string());
        assert_eq!(extract_basename(input), expected);
    }

    #[test]
    fn test_remove_path_quotes() {
        let input = "\"C:\\foo\\bar.txt\"\n\"Not a path\"\nD:\\data";
        let expected = Some("C:\\foo\\bar.txt\n\"Not a path\"\nD:\\data".to_string());
        assert_eq!(remove_path_quotes(input), expected);
    }

    #[test]
    fn test_add_path_quotes() {
        let input = "C:\\foo\\bar.txt\n\"Already quoted\"\nE:\\work";
        let expected = Some("\"C:\\foo\\bar.txt\"\n\"Already quoted\"\n\"E:\\work\"".to_string());
        assert_eq!(add_path_quotes(input), expected);
    }

    #[test]
    fn test_convert_to_forward_slash() {
        let input = "C:\\Users\\Test\\file.txt";
        let expected = Some("C:/Users/Test/file.txt".to_string());
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    #[test]
    fn test_convert_to_forward_slash_multiline() {
        let input = "C:\\foo\\bar.txt\nD:\\data\\test.log";
        let expected = Some("C:/foo/bar.txt\nD:/data/test.log".to_string());
        assert_eq!(convert_to_forward_slash(input), expected);
    }

    #[test]
    fn test_convert_to_forward_slash_already_slash() {
        let input = "/usr/local/bin";
        assert_eq!(convert_to_forward_slash(input), None);
    }

    #[test]
    fn test_convert_to_backslash() {
        let input = "/usr/local/bin";
        let expected = Some("\\usr\\local\\bin".to_string());
        assert_eq!(convert_to_backslash(input), expected);
    }

    #[test]
    fn test_convert_to_backslash_multiline() {
        let input = "/home/user/file.txt\n/tmp/test.log";
        let expected = Some("\\home\\user\\file.txt\n\\tmp\\test.log".to_string());
        assert_eq!(convert_to_backslash(input), expected);
    }

    #[test]
    fn test_convert_to_backslash_already_backslash() {
        let input = "C:\\Windows\\System32";
        assert_eq!(convert_to_backslash(input), None);
    }

    #[test]
    fn test_convert_mixed_content() {
        let input = "C:\\foo\\bar.txt\nNot a path\nD:\\data";
        let expected = Some("C:/foo/bar.txt\nNot a path\nD:/data".to_string());
        assert_eq!(convert_to_forward_slash(input), expected);
    }
}
