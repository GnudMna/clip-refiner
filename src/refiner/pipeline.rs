use super::context::RefineContext;
use super::dispatch::Refiner;
use super::mode::RefineMode;

// ======================================================================
// パイプライン分割
// ======================================================================
/// 加工パイプラインをテキスト用モード列と末尾の画像モードに分割する
///
/// 画像出力モードは末尾に1つのみ有効とみなす
///
/// # Arguments
/// * `pipeline` - 適用順の加工モード列
///
/// # Returns
/// * `(Vec<RefineMode>, Option<RefineMode>)` - テキスト用モード列と末尾の画像モード
pub fn split_pipeline(pipeline: &[RefineMode]) -> (Vec<RefineMode>, Option<RefineMode>) {
    if pipeline.is_empty() {
        return (Vec::new(), None);
    }

    if let Some(&last) = pipeline.last()
        && last.produces_image()
    {
        let text_modes = pipeline[..pipeline.len() - 1].to_vec();
        return (text_modes, Some(last));
    }

    (pipeline.to_vec(), None)
}

// ======================================================================
// テキスト加工
// ======================================================================
/// テキスト加工モード列を順に適用する
///
/// # Arguments
/// * `text` - 加工前のテキスト
/// * `modes` - 適用するテキスト加工モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Option<String>` - いずれかの段で変更があった場合は加工後テキスト、変更がなければ `None`
pub fn apply_text_pipeline(
    text: &str,
    modes: &[RefineMode],
    ctx: &RefineContext,
) -> Option<String> {
    if modes.is_empty() {
        return None;
    }

    let mut current = text.to_string();
    let mut changed = false;

    for mode in modes {
        let refined = mode.refine(&current, ctx);
        if refined.as_ref() != current.as_str() {
            current = refined.into_owned();
            changed = true;
        }
    }

    changed.then_some(current)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 空パイプラインは分割結果が空であること
    #[test]
    fn split_empty_pipeline() {
        assert_eq!(split_pipeline(&[]), (Vec::new(), None));
    }

    /// テキストのみのパイプラインは画像モードなしで返ること
    #[test]
    fn split_text_only_pipeline() {
        let pipeline = vec![RefineMode::UrlDecode, RefineMode::Trim];
        assert_eq!(split_pipeline(&pipeline), (pipeline.clone(), None));
    }

    /// 末尾の画像モードは分割されること
    #[test]
    fn split_trailing_image_mode() {
        let pipeline = vec![RefineMode::Trim, RefineMode::ExcelToImage];
        assert_eq!(
            split_pipeline(&pipeline),
            (vec![RefineMode::Trim], Some(RefineMode::ExcelToImage))
        );
    }

    /// テキスト加工パイプラインが順に適用されること
    #[test]
    fn apply_text_pipeline_chains_modes() {
        let ctx = RefineContext::default();
        let input = "  %E3%81%82  ";
        let result = apply_text_pipeline(input, &[RefineMode::UrlDecode, RefineMode::Trim], &ctx);
        assert_eq!(result, Some("あ".to_string()));
    }

    /// 変更がない場合は `None` を返すこと
    #[test]
    fn apply_text_pipeline_returns_none_when_unchanged() {
        let ctx = RefineContext::default();
        assert_eq!(
            apply_text_pipeline("hello", &[RefineMode::Trim], &ctx),
            None
        );
    }
}
