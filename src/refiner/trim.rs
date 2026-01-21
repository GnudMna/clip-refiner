/// 文字列の前後空白を削除する
pub fn trim_text(input: &str) -> String {
    input.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_text() {
        assert_eq!(trim_text("  hello  "), "hello");
        assert_eq!(trim_text("\n world \r\n"), "world");
    }
}
