//! `SemVer` / タグ文字列の正規化

use semver::Version;

/// タグやバージョン文字列から先頭の `v` / `V` を除去する
///
/// # Arguments
/// * `value` - タグまたはバージョン文字列
///
/// # Returns
/// * `&str` - プレフィックス除去後の文字列
pub(crate) fn strip_version_prefix(value: &str) -> &str {
    value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value)
}

/// `SemVer` 文字列をパースする
///
/// # Arguments
/// * `value` - `v` 付きでも可
///
/// # Returns
/// * `Result<Version, semver::Error>` - パース結果
pub(crate) fn parse_semver(value: &str) -> Result<Version, semver::Error> {
    Version::parse(strip_version_prefix(value.trim()))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::{parse_semver, strip_version_prefix};

    /// タグ先頭の `v` を除去できるようにすること
    #[test]
    fn strip_version_prefix_removes_leading_v() {
        assert_eq!(strip_version_prefix("v0.13.1"), "0.13.1");
        assert_eq!(strip_version_prefix("0.13.1"), "0.13.1");
        assert_eq!(strip_version_prefix("V1.0.0"), "1.0.0");
    }

    /// `SemVer` パースが `v` 付きを受け付けるようにすること
    #[test]
    fn parse_semver_accepts_v_prefix() {
        let version = parse_semver("v1.2.3").expect("semver");
        assert_eq!(version.to_string(), "1.2.3");
    }
}
