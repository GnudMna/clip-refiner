use std::borrow::Cow;

// ======================================================================
// 定数
// ======================================================================
/// 全角英数記号 (U+FF01..=U+FF5E) と ASCII 印字可能文字 (U+0021..=U+007E) のコードポイント差
const FULLWIDTH_ASCII_OFFSET: u32 = 0xFEE0;

/// 全角スペース
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

/// 全角円記号 (`￥`)
const FULLWIDTH_YEN: char = '\u{FFE5}';

/// 半角円記号 (`¥`)
const HALFWIDTH_YEN: char = '\u{00A5}';

/// 半角カナ領域の先頭 (`｡`)
const HALFWIDTH_KANA_START: u32 = 0xFF61;

/// 半角濁点
const HALFWIDTH_VOICED_MARK: char = 'ﾞ';

/// 半角半濁点
const HALFWIDTH_SEMI_VOICED_MARK: char = 'ﾟ';

/// 半角カナ (U+FF61..=U+FF9F) に対応する全角文字 (添字は `HALFWIDTH_KANA_START` からの差)
const HALFWIDTH_KANA_TO_FULLWIDTH: [char; 63] = [
    '。', '「', '」', '、', '・', 'ヲ', 'ァ', 'ィ', 'ゥ', 'ェ', 'ォ', 'ャ', 'ュ', 'ョ', 'ッ', 'ー',
    'ア', 'イ', 'ウ', 'エ', 'オ', 'カ', 'キ', 'ク', 'ケ', 'コ', 'サ', 'シ', 'ス', 'セ', 'ソ', 'タ',
    'チ', 'ツ', 'テ', 'ト', 'ナ', 'ニ', 'ヌ', 'ネ', 'ノ', 'ハ', 'ヒ', 'フ', 'ヘ', 'ホ', 'マ', 'ミ',
    'ム', 'メ', 'モ', 'ヤ', 'ユ', 'ヨ', 'ラ', 'リ', 'ル', 'レ', 'ロ', 'ワ', 'ン', '゛', '゜',
];

// ======================================================================
// パブリック関数
// ======================================================================
/// 全角の英数字・記号・スペース・カタカナを半角へ変換する
///
/// 濁音・半濁音は清音と濁点・半濁点の 2 文字に分解する (例: `ガ` → `ｶﾞ`)
/// 半角に対応する文字が無いもの (ひらがな・`ヵ` など) はそのまま残す
///
/// # Arguments
/// * `input` - 変換対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列への参照を返す
pub fn to_halfwidth(input: &str) -> Cow<'_, str> {
    if !input.chars().any(has_halfwidth) {
        return Cow::Borrowed(input);
    }

    let mut result = String::with_capacity(input.len());
    for c in input.chars() {
        if let Some(ascii) = fullwidth_ascii_to_halfwidth(c) {
            result.push(ascii);
        } else if let Some((base, mark)) = decompose_voiced(c) {
            match fullwidth_to_halfwidth_kana(base) {
                Some(kana) => {
                    result.push(kana);
                    result.push(mark);
                }
                None => result.push(c),
            }
        } else {
            result.push(fullwidth_to_halfwidth_kana(c).unwrap_or(c));
        }
    }
    Cow::Owned(result)
}

/// 半角の英数字・記号・スペース・カタカナを全角へ変換する
///
/// 半角カナの直後の濁点・半濁点は 1 文字に合成する (例: `ｶﾞ` → `ガ`)
/// 合成できない濁点・半濁点は単独の全角文字 (`゛` / `゜`) にする
///
/// # Arguments
/// * `input` - 変換対象の文字列
///
/// # Returns
/// * `Cow<'_, str>` - 変換後の文字列。変更がない場合は元の文字列への参照を返す
pub fn to_fullwidth(input: &str) -> Cow<'_, str> {
    if !input.chars().any(has_fullwidth) {
        return Cow::Borrowed(input);
    }

    let mut result = String::with_capacity(input.len() * 3);
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(full) = halfwidth_ascii_to_fullwidth(c) {
            result.push(full);
            continue;
        }
        let Some(full) = halfwidth_kana_to_fullwidth(c) else {
            result.push(c);
            continue;
        };
        let composed = chars.peek().and_then(|&mark| compose_voiced(full, mark));
        if let Some(composed) = composed {
            chars.next();
            result.push(composed);
        } else {
            result.push(full);
        }
    }
    Cow::Owned(result)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 半角へ変換できる文字かどうか
fn has_halfwidth(c: char) -> bool {
    fullwidth_ascii_to_halfwidth(c).is_some()
        || fullwidth_to_halfwidth_kana(c).is_some()
        || decompose_voiced(c).is_some()
}

/// 全角へ変換できる文字かどうか
fn has_fullwidth(c: char) -> bool {
    halfwidth_ascii_to_fullwidth(c).is_some() || halfwidth_kana_to_fullwidth(c).is_some()
}

/// 全角英数記号・全角スペースを半角へ変換する
fn fullwidth_ascii_to_halfwidth(c: char) -> Option<char> {
    match c {
        IDEOGRAPHIC_SPACE => Some(' '),
        FULLWIDTH_YEN => Some(HALFWIDTH_YEN),
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - FULLWIDTH_ASCII_OFFSET),
        _ => None,
    }
}

/// ASCII 印字可能文字・スペースを全角へ変換する
fn halfwidth_ascii_to_fullwidth(c: char) -> Option<char> {
    match c {
        ' ' => Some(IDEOGRAPHIC_SPACE),
        HALFWIDTH_YEN => Some(FULLWIDTH_YEN),
        '!'..='~' => char::from_u32(c as u32 + FULLWIDTH_ASCII_OFFSET),
        _ => None,
    }
}

/// 半角カナを対応する全角文字へ変換する
fn halfwidth_kana_to_fullwidth(c: char) -> Option<char> {
    let index = (c as u32).checked_sub(HALFWIDTH_KANA_START)?;
    HALFWIDTH_KANA_TO_FULLWIDTH.get(index as usize).copied()
}

/// 全角カナ・約物を対応する半角カナへ変換する
fn fullwidth_to_halfwidth_kana(c: char) -> Option<char> {
    let index = HALFWIDTH_KANA_TO_FULLWIDTH
        .iter()
        .position(|&full| full == c)?;
    char::from_u32(HALFWIDTH_KANA_START + u32::try_from(index).ok()?)
}

/// 濁音・半濁音の全角カタカナを清音と半角の濁点・半濁点へ分解する
///
/// # Arguments
/// * `c` - 分解対象の文字
///
/// # Returns
/// * `Option<(char, char)>` - 清音の全角カタカナと半角の濁点・半濁点。濁音・半濁音でない場合は `None`
fn decompose_voiced(c: char) -> Option<(char, char)> {
    let offset_base = |offset: u32| char::from_u32(c as u32 - offset);
    match c {
        'ガ' | 'ギ' | 'グ' | 'ゲ' | 'ゴ' | 'ザ' | 'ジ' | 'ズ' | 'ゼ' | 'ゾ' | 'ダ' | 'ヂ'
        | 'ヅ' | 'デ' | 'ド' | 'バ' | 'ビ' | 'ブ' | 'ベ' | 'ボ' => {
            Some((offset_base(1)?, HALFWIDTH_VOICED_MARK))
        }
        'パ' | 'ピ' | 'プ' | 'ペ' | 'ポ' => {
            Some((offset_base(2)?, HALFWIDTH_SEMI_VOICED_MARK))
        }
        'ヴ' => Some(('ウ', HALFWIDTH_VOICED_MARK)),
        'ヷ' => Some(('ワ', HALFWIDTH_VOICED_MARK)),
        'ヺ' => Some(('ヲ', HALFWIDTH_VOICED_MARK)),
        _ => None,
    }
}

/// 全角カタカナと半角の濁点・半濁点を 1 文字の濁音・半濁音へ合成する
///
/// `decompose_voiced` の逆変換として、分解結果が一致する候補を探す
///
/// # Arguments
/// * `base` - 清音の全角カタカナ
/// * `mark` - 直後の文字 (半角の濁点・半濁点であれば合成対象)
///
/// # Returns
/// * `Option<char>` - 合成した文字。合成できない場合は `None`
fn compose_voiced(base: char, mark: char) -> Option<char> {
    if mark != HALFWIDTH_VOICED_MARK && mark != HALFWIDTH_SEMI_VOICED_MARK {
        return None;
    }
    let shifted = [1, 2].map(|offset| char::from_u32(base as u32 + offset));
    shifted
        .into_iter()
        .flatten()
        .chain(['ヴ', 'ヷ', 'ヺ'])
        .find(|&candidate| decompose_voiced(candidate) == Some((base, mark)))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 全角英数字・記号・スペースを半角へ変換すること
    #[test]
    fn test_to_halfwidth_ascii() {
        assert_eq!(to_halfwidth("ＡＢＣ　ａｂｃ　０１２"), "ABC abc 012");
        assert_eq!(to_halfwidth("！＂＃＄％＆（）～＼"), "!\"#$%&()~\\");
    }

    /// 全角円記号を半角円記号へ変換すること
    #[test]
    fn test_to_halfwidth_yen() {
        assert_eq!(to_halfwidth("￥１００"), "¥100");
    }

    /// 半角円記号を全角円記号へ変換すること
    #[test]
    fn test_to_fullwidth_yen() {
        assert_eq!(to_fullwidth("¥100"), "￥１００");
    }

    /// 全角カタカナと約物を半角へ変換し、濁音・半濁音を分解すること
    #[test]
    fn test_to_halfwidth_kana() {
        assert_eq!(to_halfwidth("カタカナ"), "ｶﾀｶﾅ");
        assert_eq!(to_halfwidth("ガギグゲゴ"), "ｶﾞｷﾞｸﾞｹﾞｺﾞ");
        assert_eq!(to_halfwidth("パピプペポ"), "ﾊﾟﾋﾟﾌﾟﾍﾟﾎﾟ");
        assert_eq!(to_halfwidth("ヴァイオリン"), "ｳﾞｧｲｵﾘﾝ");
        assert_eq!(to_halfwidth("ヷヺ"), "ﾜﾞｦﾞ");
        assert_eq!(to_halfwidth("「データ」、コピー。・"), "｢ﾃﾞｰﾀ｣､ｺﾋﾟｰ｡･");
        assert_eq!(to_halfwidth("゛゜"), "ﾞﾟ");
    }

    /// 半角に対応する文字が無いものはそのまま残すようにすること
    #[test]
    fn test_to_halfwidth_keeps_unmapped() {
        assert_eq!(to_halfwidth("ひらがなと漢字"), "ひらがなと漢字");
        assert_eq!(to_halfwidth("ヵヶヰヱヮ"), "ヵヶヰヱヮ");
        assert_eq!(to_halfwidth("ｶﾀｶﾅ abc"), "ｶﾀｶﾅ abc");
    }

    /// 変換対象が無い場合は元の文字列を借用すること
    #[test]
    fn test_to_halfwidth_borrowed_when_unchanged() {
        assert!(matches!(to_halfwidth("abc あいう"), Cow::Borrowed(_)));
        assert!(matches!(to_halfwidth(""), Cow::Borrowed(_)));
    }

    /// 半角英数字・記号・スペースを全角へ変換すること
    #[test]
    fn test_to_fullwidth_ascii() {
        assert_eq!(to_fullwidth("ABC abc 012"), "ＡＢＣ　ａｂｃ　０１２");
        assert_eq!(to_fullwidth("!\"#$%&()~\\"), "！＂＃＄％＆（）～＼");
    }

    /// 半角カナを全角へ変換し、濁点・半濁点を合成すること
    #[test]
    fn test_to_fullwidth_kana() {
        assert_eq!(to_fullwidth("ｶﾀｶﾅ"), "カタカナ");
        assert_eq!(to_fullwidth("ｶﾞｷﾞｸﾞｹﾞｺﾞ"), "ガギグゲゴ");
        assert_eq!(to_fullwidth("ﾊﾟﾋﾟﾌﾟﾍﾟﾎﾟ"), "パピプペポ");
        assert_eq!(to_fullwidth("ｳﾞｧｲｵﾘﾝ"), "ヴァイオリン");
        assert_eq!(to_fullwidth("ﾜﾞｦﾞ"), "ヷヺ");
        assert_eq!(to_fullwidth("｢ﾃﾞｰﾀ｣､ｺﾋﾟｰ｡･"), "「データ」、コピー。・");
    }

    /// 合成できない濁点・半濁点は単独の全角文字にすること
    #[test]
    fn test_to_fullwidth_standalone_marks() {
        assert_eq!(to_fullwidth("ｱﾞ"), "ア゛");
        assert_eq!(to_fullwidth("ｶﾟ"), "カ゜");
        assert_eq!(to_fullwidth("ﾞﾟ"), "゛゜");
        assert_eq!(to_fullwidth("ｶﾞﾞ"), "ガ゛");
    }

    /// 全角に対応する文字が無いものはそのまま残すようにすること
    #[test]
    fn test_to_fullwidth_keeps_unmapped() {
        assert_eq!(to_fullwidth("ab\tc\r\n"), "ａｂ\tｃ\r\n");
        assert_eq!(to_fullwidth("全角カナ"), "全角カナ");
    }

    /// 変換対象が無い場合は元の文字列を借用すること
    #[test]
    fn test_to_fullwidth_borrowed_when_unchanged() {
        assert!(matches!(to_fullwidth("ＡＢＣ　あいう"), Cow::Borrowed(_)));
        assert!(matches!(to_fullwidth("\r\n"), Cow::Borrowed(_)));
    }

    /// 半角カナ領域の全文字が往復変換で元に戻るようにすること
    #[test]
    fn test_halfwidth_kana_round_trip() {
        let halfwidth: String = (0xFF61..=0xFF9D).filter_map(char::from_u32).collect();
        assert_eq!(to_halfwidth(&to_fullwidth(&halfwidth)), halfwidth);
    }
}
