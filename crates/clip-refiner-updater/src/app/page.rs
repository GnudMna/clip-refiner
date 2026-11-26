//! 更新画面の各ページの内容
//!
//! 文言・ボタン・閉じ方だけを決め、表示方法 (`dialog`) には依存しない

// ======================================================================
// 操作
// ======================================================================
/// 画面で選べる操作 (閉じる・キャンセル以外)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// 更新を始める
    Update,
    /// アプリを起動する
    Start,
    /// ログのフォルダを開く (ページは切り替わらない)
    OpenLog,
    /// 画面を閉じる
    Close,
}

/// 閉じるボタン (`×`・`Esc` を含む) の扱い
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dismiss {
    /// 画面を閉じる
    Close,
    /// まだ何も変更していない処理を打ち切って閉じる
    Abort,
    /// 処理へ中止を伝え、結果が出るまで閉じない
    RequestCancel,
    /// 閉じられない
    Denied,
}

impl Dismiss {
    /// 閉じるボタンの表示名を返す
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Close | Self::Denied => "閉じる",
            Self::Abort | Self::RequestCancel => "キャンセル",
        }
    }
}

// ======================================================================
// 表示内容
// ======================================================================
/// 見出しの横に出すアイコン
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    /// 情報
    Information,
    /// 警告
    Warning,
    /// エラー
    Error,
}

/// 進捗バーの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Progress {
    /// 表示しない
    Hidden,
    /// 割合を表示する
    Percent,
    /// 終わりの分からない処理として流し続ける
    Marquee,
}

/// 1 ページ分の表示内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PageSpec {
    /// 見出し
    pub instruction: String,
    /// 本文
    pub content: String,
    /// アイコン
    pub icon: Icon,
    /// 進捗バー
    pub progress: Progress,
    /// 閉じるボタン以外のボタン (左から順)
    pub buttons: Vec<(Action, &'static str)>,
    /// 閉じるボタンの扱い
    pub dismiss: Dismiss,
    /// 更新の結果を伝えるページか (利用者の注意を引く)
    pub is_result: bool,
}

/// 更新後のアプリの状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AppStatus {
    /// インストールされている版 (読み取れない場合は `None`)
    pub version: Option<String>,
    /// 動作しているか
    pub running: bool,
}

impl AppStatus {
    /// 状態の説明文を返す
    fn describe(&self) -> String {
        let version = self
            .version
            .as_ref()
            .map_or_else(String::new, |version| format!(" {version}"));
        if self.running {
            format!("ClipRefiner{version} が動作しています。")
        } else {
            format!("ClipRefiner{version} は終了しています。")
        }
    }

    /// 停止中に選べる「起動」ボタンを先頭に付けた、失敗時のボタンを返す
    fn failure_buttons(&self) -> Vec<(Action, &'static str)> {
        let mut buttons = vec![(Action::OpenLog, "ログを開く")];
        if !self.running {
            buttons.insert(0, (Action::Start, "ClipRefiner を起動"));
        }
        buttons
    }
}

// ======================================================================
// ページ
// ======================================================================
/// 更新画面のページ
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Page {
    /// 最新版を確認している
    Checking,
    /// 最新版を使っている
    UpToDate {
        /// 現在の版
        current: String,
    },
    /// 確認に失敗した
    CheckFailed {
        /// 理由
        reason: String,
    },
    /// 更新できる
    Available {
        /// 現在の版
        current: String,
        /// 更新先の版
        latest: String,
        /// ダウンロードサイズ (バイト)
        size: u64,
    },
    /// ダウンロードと検証をしている
    Downloading {
        /// 更新先の版
        latest: String,
        /// ダウンロードサイズ (バイト)
        size: u64,
    },
    /// アプリの終了を待っている
    WaitingExit,
    /// インストールしている
    Installing {
        /// 更新先の版
        latest: String,
    },
    /// アプリの起動を待っている
    Starting,
    /// 更新が完了し、新しい版が動作している
    Succeeded {
        /// 更新先の版
        latest: String,
    },
    /// インストールは完了したが、新しい版の動作を確認できない
    StartFailed {
        /// 更新先の版
        latest: String,
        /// 理由
        reason: String,
    },
    /// 完了に Windows の再起動が必要
    RebootRequired {
        /// 更新先の版
        latest: String,
        /// アプリの状態
        status: AppStatus,
    },
    /// 利用者が中止した
    Canceled {
        /// アプリの状態
        status: AppStatus,
    },
    /// 更新に失敗した
    Failed {
        /// 理由
        reason: String,
        /// アプリの状態
        status: AppStatus,
    },
    /// 更新処理の内部エラー
    InternalError {
        /// 理由
        reason: String,
    },
}

impl Page {
    /// 表示内容を組み立てる
    pub(super) fn spec(&self) -> PageSpec {
        match self {
            Self::Checking => busy("最新版を確認しています", "", Dismiss::Abort),
            Self::UpToDate { current } => notice(
                Icon::Information,
                "最新版です",
                format!("ClipRefiner {current} をお使いです。"),
                Vec::new(),
            ),
            Self::CheckFailed { reason } => notice(
                Icon::Error,
                "更新を確認できませんでした",
                reason.clone(),
                vec![(Action::OpenLog, "ログを開く")],
            ),
            Self::Available {
                current,
                latest,
                size,
            } => notice(
                Icon::Information,
                format!("ClipRefiner {latest} に更新できます"),
                format!(
                    "現在のバージョン: {current}\nダウンロードサイズ: {}\n\n更新中は ClipRefiner を終了し、完了後に自動で起動します。",
                    format_mib(*size)
                ),
                vec![(Action::Update, "更新")],
            ),
            Self::Downloading { latest, size } => PageSpec {
                progress: Progress::Percent,
                ..busy(
                    format!("ClipRefiner {latest} をダウンロードしています"),
                    progress_text(0, *size),
                    Dismiss::Abort,
                )
            },
            Self::WaitingExit => busy(
                "ClipRefiner の終了を待っています",
                "",
                Dismiss::RequestCancel,
            ),
            Self::Installing { latest } => busy(
                format!("ClipRefiner {latest} をインストールしています"),
                "完了するまでこの画面は閉じられません。",
                Dismiss::Denied,
            ),
            Self::Starting => busy("ClipRefiner の起動を確認しています", "", Dismiss::Denied),
            Self::Succeeded { latest } => result(
                Icon::Information,
                format!("ClipRefiner {latest} に更新しました"),
                "タスクトレイで動作しています。".to_string(),
                Vec::new(),
            ),
            Self::StartFailed { latest, reason } => result(
                Icon::Warning,
                format!("ClipRefiner {latest} をインストールしましたが、動作を確認できません"),
                reason.clone(),
                vec![
                    (Action::Start, "もう一度起動"),
                    (Action::OpenLog, "ログを開く"),
                ],
            ),
            Self::RebootRequired { latest, status } => result(
                Icon::Warning,
                "Windows の再起動が必要です".to_string(),
                format!(
                    "Windows を再起動すると ClipRefiner {latest} への更新が完了します。\n\n現在は {}",
                    status.describe()
                ),
                status.failure_buttons(),
            ),
            Self::Canceled { status } => result(
                Icon::Warning,
                "更新をキャンセルしました".to_string(),
                status.describe(),
                status.failure_buttons(),
            ),
            Self::Failed { reason, status } => result(
                Icon::Error,
                "更新できませんでした".to_string(),
                format!("{reason}\n\n{}", status.describe()),
                status.failure_buttons(),
            ),
            Self::InternalError { reason } => result(
                Icon::Error,
                "更新できませんでした".to_string(),
                format!(
                    "{reason}\n\nClipRefiner が動作していない場合は、起動してバージョンを確認してください。"
                ),
                vec![(Action::OpenLog, "ログを開く")],
            ),
        }
    }
}

// ======================================================================
// 組み立て
// ======================================================================
/// 処理中のページを組み立てる
fn busy(instruction: impl Into<String>, content: impl Into<String>, dismiss: Dismiss) -> PageSpec {
    PageSpec {
        instruction: instruction.into(),
        content: content.into(),
        icon: Icon::Information,
        progress: Progress::Marquee,
        buttons: Vec::new(),
        dismiss,
        is_result: false,
    }
}

/// 利用者の操作を待つページを組み立てる
fn notice(
    icon: Icon,
    instruction: impl Into<String>,
    content: String,
    buttons: Vec<(Action, &'static str)>,
) -> PageSpec {
    PageSpec {
        instruction: instruction.into(),
        content,
        icon,
        progress: Progress::Hidden,
        buttons,
        dismiss: Dismiss::Close,
        is_result: false,
    }
}

/// 更新の結果を伝えるページを組み立てる
fn result(
    icon: Icon,
    instruction: String,
    content: String,
    buttons: Vec<(Action, &'static str)>,
) -> PageSpec {
    PageSpec {
        is_result: true,
        ..notice(icon, instruction, content, buttons)
    }
}

// ======================================================================
// 進捗
// ======================================================================
/// ダウンロード中の本文を返す
pub(super) fn progress_text(received: u64, total: u64) -> String {
    if received >= total && total > 0 {
        return "ダウンロードしたファイルを検証しています。".to_string();
    }
    format!("{} / {}", format_mib(received), format_mib(total))
}

/// 進捗バーの位置 (0〜100) を返す
pub(super) fn percent(received: u64, total: u64) -> u16 {
    if total == 0 {
        return 0;
    }
    let percent = u128::from(received.min(total)) * 100 / u128::from(total);
    u16::try_from(percent).unwrap_or(100)
}

/// バイト数を MB 表記 (小数 1 桁) にする
fn format_mib(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    let tenths = bytes.saturating_mul(10).div_ceil(MIB);
    format!("{}.{} MB", tenths / 10, tenths % 10)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn status(running: bool) -> AppStatus {
        AppStatus {
            version: Some("4.0.1".to_string()),
            running,
        }
    }

    /// インストール中と起動確認中は閉じられないこと
    #[test]
    fn irreversible_pages_cannot_be_dismissed() {
        let installing = Page::Installing {
            latest: "4.1.0".to_string(),
        };
        assert_eq!(installing.spec().dismiss, Dismiss::Denied);
        assert_eq!(Page::Starting.spec().dismiss, Dismiss::Denied);
    }

    /// 変更を加える前のページは、その場で打ち切れること
    #[test]
    fn harmless_pages_can_be_aborted() {
        let downloading = Page::Downloading {
            latest: "4.1.0".to_string(),
            size: 10,
        };
        assert_eq!(Page::Checking.spec().dismiss, Dismiss::Abort);
        assert_eq!(downloading.spec().dismiss, Dismiss::Abort);
        assert_eq!(Page::WaitingExit.spec().dismiss, Dismiss::RequestCancel);
    }

    /// 成功のページが更新先の版と、動作している場所を示すこと
    #[test]
    fn success_page_tells_version_and_location() {
        let spec = Page::Succeeded {
            latest: "4.1.0".to_string(),
        }
        .spec();
        assert!(spec.instruction.contains("4.1.0"));
        assert!(spec.content.contains("タスクトレイ"));
        assert!(spec.is_result);
    }

    /// 失敗のページが理由と現在の版を示し、停止中だけ起動ボタンを出すこと
    #[test]
    fn failure_page_tells_reason_and_status() {
        let page = |running| {
            Page::Failed {
                reason: "理由".to_string(),
                status: status(running),
            }
            .spec()
        };

        let running = page(true);
        assert!(running.content.contains("理由"));
        assert!(running.content.contains("4.0.1 が動作しています"));
        assert_eq!(running.buttons, [(Action::OpenLog, "ログを開く")]);

        let stopped = page(false);
        assert!(stopped.content.contains("4.0.1 は終了しています"));
        assert_eq!(stopped.buttons[0].0, Action::Start);
    }

    /// 再起動待ちを成功と区別して表示すること
    #[test]
    fn reboot_page_is_not_success() {
        let spec = Page::RebootRequired {
            latest: "4.1.0".to_string(),
            status: status(true),
        }
        .spec();
        assert_eq!(spec.icon, Icon::Warning);
        assert!(spec.content.contains("Windows を再起動"));
    }

    /// 進捗の割合が 0〜100 に収まること
    #[test]
    fn percent_is_clamped() {
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(1, 4), 25);
        assert_eq!(percent(5, 4), 100);
        assert_eq!(percent(u64::MAX, u64::MAX), 100);
    }

    /// 受信が終わると検証中の文言へ変わること
    #[test]
    fn progress_text_switches_to_verifying() {
        assert_eq!(
            progress_text(1024 * 1024, 2 * 1024 * 1024),
            "1.0 MB / 2.0 MB"
        );
        assert!(progress_text(10, 10).contains("検証"));
    }
}
