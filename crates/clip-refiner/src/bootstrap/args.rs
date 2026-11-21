//! CLI 引数定義とワンショット判定

use std::path::PathBuf;
use std::str::FromStr;

use crate::refiner::RefineMode;
use crate::tray::E2eWindow;

use clap::Parser;

// ======================================================================
// 引数設定
// ======================================================================
/// コマンドライン引数
///
/// アプリケーションの動作モード(常駐またはワンショット)を指定する
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "クリップボードのテキストを加工するツール",
    help_template = "\
{about-with-newline}
使用方法:
    引数なし: システムトレイに常駐し、クリップボードを監視して自動加工
    --mode / --pipeline: クリップボードの内容を一度だけ加工 (常駐中は IPC 委譲)

{all-args}
"
)]
pub(super) struct Args {
    /// 実行モードの指定(ワンショット実行用)
    #[arg(short = 'm', long = "mode", value_parser = parse_refine_mode)]
    pub(super) mode: Option<RefineMode>,
    /// ワンショットで順に適用する変換モード列 (`--mode` より優先)
    #[arg(long = "pipeline", value_parser = parse_refine_mode, num_args = 1..)]
    pub(super) pipeline: Vec<RefineMode>,
    /// 正規表現パターン (`config.toml` の有効プリセットの代わりに 1 ステップとして上書き)
    #[arg(long = "regex-pattern")]
    pub(super) regex_pattern: Option<String>,
    /// 正規表現の置換文字列 (ワンショットの 1 ステップ上書き)
    #[arg(long = "regex-replacement")]
    pub(super) regex_replacement: Option<String>,
    /// 正規表現で大文字小文字を無視する
    #[arg(long = "regex-case-insensitive")]
    pub(super) regex_case_insensitive: bool,
    /// 正規表現で複数行モードを有効にする
    #[arg(long = "regex-multiline")]
    pub(super) regex_multiline: bool,
    /// 正規表現プリセット名 (`config.toml` の `regex.presets[].name` で先頭一致)
    #[arg(long = "regex-preset")]
    pub(super) regex_preset: Option<String>,
    /// E2E 用: 指定 `WebView` を自動表示して常駐する (Playwright + CDP)
    #[arg(
        long = "e2e-window",
        value_enum,
        conflicts_with_all = ["mode", "pipeline", "apply_portable_update"]
    )]
    pub(super) e2e_window: Option<E2eWindow>,
    /// ポータブル更新の差し替えヘルパー (内部用)
    #[arg(
        long = "apply-portable-update",
        hide = true,
        conflicts_with_all = ["mode", "pipeline", "e2e_window"]
    )]
    pub(super) apply_portable_update: bool,
    /// 差し替え前に終了を待つ旧プロセス PID (内部用)
    #[arg(long = "wait-pid", hide = true, requires = "apply_portable_update")]
    pub(super) wait_pid: Option<u32>,
    /// 差し替え先ディレクトリ (内部用)
    #[arg(long = "install-dir", hide = true, requires = "apply_portable_update")]
    pub(super) install_dir: Option<PathBuf>,
    /// 差し替え後に削除する一時ディレクトリ (内部用)
    #[arg(long = "cleanup-dir", hide = true, requires = "apply_portable_update")]
    pub(super) cleanup_dir: Option<PathBuf>,
}

/// ワンショット実行 (`--mode` / `--pipeline`) かどうかを判定する
pub(super) fn is_oneshot(args: &Args) -> bool {
    args.mode.is_some() || !args.pipeline.is_empty()
}

/// CLI 引数の変換モード名 (kebab-case) を `RefineMode` へ変換する
fn parse_refine_mode(value: &str) -> Result<RefineMode, String> {
    RefineMode::from_str(value).map_err(|_| format!("'{value}' は有効な変換モードではありません"))
}

/// テスト用の代表的な CLI 引数を返す
#[cfg(test)]
pub(super) fn sample_args_for_test() -> Args {
    Args {
        mode: Some(RefineMode::Trim),
        pipeline: Vec::new(),
        regex_pattern: None,
        regex_replacement: None,
        regex_case_insensitive: false,
        regex_multiline: false,
        regex_preset: None,
        e2e_window: None,
        apply_portable_update: false,
        wait_pid: None,
        install_dir: None,
        cleanup_dir: None,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::{Args, is_oneshot, sample_args_for_test};
    use crate::refiner::RefineMode;

    /// `--mode` 指定時はワンショットと判定すること
    #[test]
    fn is_oneshot_when_mode_is_set() {
        assert!(is_oneshot(&sample_args_for_test()));
    }

    /// `--pipeline` 指定時はワンショットと判定すること
    #[test]
    fn is_oneshot_when_pipeline_is_set() {
        let args = Args {
            mode: None,
            pipeline: vec![RefineMode::Trim],
            ..sample_args_for_test()
        };
        assert!(is_oneshot(&args));
    }

    /// モード未指定時は常駐起動と判定すること
    #[test]
    fn is_oneshot_false_for_tray_launch() {
        let args = Args {
            mode: None,
            pipeline: Vec::new(),
            ..sample_args_for_test()
        };
        assert!(!is_oneshot(&args));
    }
}
