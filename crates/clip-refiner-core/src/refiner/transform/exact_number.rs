//! 10 進の数値表記が倍精度浮動小数点 (`f64`) への丸めで桁を失うかの判定
//!
//! YAML のパーサと直列化先は数値を `f64` でしか扱えないため、
//! 丸めで値が変わる数値だけ元の表記を保つ判断に使う

use serde_json::Number;

// ======================================================================
// 型定義
// ======================================================================
/// 10 進表記を符号・有効数字・小数点位置へ正規化した値
///
/// 値は `0.{digits} × 10^point` を表す。有効数字の前後の 0 は取り除く
#[derive(Debug, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    digits: String,
    point: i64,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 数値の元の表記が `f64` に丸めると別の値になるか判定する
///
/// 丸め後の値は JSON の数値表記 (最短の往復表記) に直して比較する
/// 元の表記が 10 進数として読めない場合 (`0x1F` など) は丸めの影響を判定できないため `false` を返す
///
/// # Arguments
/// * `text` - 元の数値表記
/// * `value` - `text` を `f64` に丸めた値
///
/// # Returns
/// * `bool` - 丸めで桁が変わる場合は `true`
pub(crate) fn loses_digits_as_f64(text: &str, value: f64) -> bool {
    let Some(original) = parse_decimal(text) else {
        return false;
    };
    let Some(rounded) = Number::from_f64(value).and_then(|n| parse_decimal(n.as_str())) else {
        return false;
    };
    original != rounded
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 10 進表記を `Decimal` へ正規化する
///
/// # Arguments
/// * `text` - 符号・小数部・指数部を持ちうる 10 進表記
///
/// # Returns
/// * `Option<Decimal>` - 正規化した値。10 進数として読めない場合は `None`
fn parse_decimal(text: &str) -> Option<Decimal> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], rest[i + 1..].parse::<i64>().ok()?),
        None => (rest, 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }

    let all_digits = format!("{int}{frac}");
    let trimmed_start = all_digits.trim_start_matches('0');
    let leading_zeros = all_digits.len() - trimmed_start.len();
    let digits = trimmed_start.trim_end_matches('0').to_owned();
    if digits.is_empty() {
        // 0 は符号と指数によらず同じ値
        return Some(Decimal {
            negative: false,
            digits,
            point: 0,
        });
    }
    let point = i64::try_from(int.len()).ok()? + exponent - i64::try_from(leading_zeros).ok()?;
    Some(Decimal {
        negative,
        digits,
        point,
    })
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn lossy(text: &str) -> bool {
        loses_digits_as_f64(text, text.parse::<f64>().expect("f64 として読める表記"))
    }

    /// 倍精度で表せる数値は丸めで桁が変わらないと判定すること
    #[test]
    fn exact_numbers_are_not_lossy() {
        for text in [
            "0",
            "1",
            "-1",
            "1.5",
            "0.1",
            "1e3",
            "1E+3",
            "1.50",
            "100",
            "0.000",
            "-0.0",
            "1e-7",
            "+2.5",
            "9007199254740992",
        ] {
            assert!(!lossy(text), "{text}");
        }
    }

    /// 丸めで桁が変わる数値を検出すること
    #[test]
    fn rounded_numbers_are_lossy() {
        for text in [
            "123456789012345678901234567890123456789012",
            "3.141592653589793238462643",
            "9007199254740993",
            "0.1234567890123456789",
            "1.00000000000000000001e5",
            "-12345678901234567890123",
        ] {
            assert!(lossy(text), "{text}");
        }
    }

    /// 10 進数として読めない表記は判定の対象外とすること
    #[test]
    fn non_decimal_text_is_not_lossy() {
        for text in ["0x1F", "1_000", ".inf", "", "-", "1e", "abc"] {
            assert!(!loses_digits_as_f64(text, 1.0), "{text}");
        }
    }
}
