//! アプリ更新の型付きエラー
//!
//! ユーザー向け文言は `Display` (`#[error(...)]`) に集約し、
//! 更新画面 / 通知 / `MessageBox` へは `to_string()` で渡す

use thiserror::Error;

// ======================================================================
// エラー型
// ======================================================================
/// 更新確認・ダウンロード・適用・差し替えの失敗
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UpdateError {
    // ----------------------------------------------------------------------
    // 確認 (GitHub API / アセット選定)
    // ----------------------------------------------------------------------
    /// 現在バージョンの `SemVer` 解釈失敗
    #[error("現在のバージョンを解釈できません: {version}")]
    CurrentVersionInvalid {
        /// 解釈できなかった文字列
        version: String,
    },
    /// 最新タグの `SemVer` 解釈失敗
    #[error("最新バージョンを解釈できません: {tag}")]
    LatestVersionInvalid {
        /// 解釈できなかったタグ
        tag: String,
    },
    /// 求める配布物種別のアセットが無い
    #[error("最新リリースに{kind_label}アセットがありません (tag={tag})")]
    AssetMissing {
        /// 表示用の種別ラベル (`MSI` / `ポータブル ZIP`)
        kind_label: String,
        /// リリースタグ
        tag: String,
    },
    /// 種別はあるが CPU 向けアセットが無い
    #[error("最新リリースにこの CPU ({arch}) 向けの{kind_label}アセットがありません (tag={tag})")]
    AssetArchMissing {
        /// CPU アーキテクチャ文字列
        arch: String,
        /// 表示用の種別ラベル
        kind_label: String,
        /// リリースタグ
        tag: String,
    },
    /// アセット名が単一のファイル名として安全に扱えない
    #[error("最新リリースのアセット名が不正なため更新できません")]
    AssetNameInvalid,
    /// アセットに digest が無い
    #[error("最新リリースにアセットの完全性情報 (digest) がありません (asset={asset})")]
    DigestMissing {
        /// アセット名
        asset: String,
    },
    /// GitHub API の回数制限などによる拒否 (`403` / `429`)
    #[error("更新情報の取得が拒否されました (HTTP {status})。時間をおいて再実行してください")]
    CheckRateLimited {
        /// HTTP ステータスコード
        status: u16,
    },
    /// GitHub API の HTTP ステータスエラー
    #[error("更新情報の取得に失敗しました (HTTP {status})")]
    CheckHttp {
        /// HTTP ステータスコード
        status: u16,
    },
    /// GitHub API のネットワーク失敗
    #[error("更新情報の取得に失敗しました。ネットワーク接続を確認してください")]
    CheckNetwork,
    /// GitHub API の JSON 解釈失敗
    #[error("更新情報の形式が不正です")]
    InvalidJson,

    // ----------------------------------------------------------------------
    // 完全性 / ダウンロード
    // ----------------------------------------------------------------------
    /// digest が空
    #[error("アセットの完全性情報 (digest) が空です")]
    DigestEmpty,
    /// digest の形式が不正
    #[error("アセットの完全性情報 (digest) の形式が不正です: {raw}")]
    DigestInvalid {
        /// 生の digest 文字列
        raw: String,
    },
    /// 宣言サイズが 0
    #[error("更新アセットのサイズ情報が不正です (0 バイト)")]
    SizeZero,
    /// 宣言サイズが絶対上限を超過
    #[error("更新アセットのサイズが上限 ({max} バイト) を超えています")]
    SizeExceedsMax {
        /// 絶対上限バイト数
        max: u64,
    },
    /// ダウンロードの HTTP ステータスエラー
    #[error("更新ファイルのダウンロードに失敗しました (HTTP {status})")]
    DownloadHttp {
        /// HTTP ステータスコード
        status: u16,
    },
    /// ダウンロードの一般失敗 (読み取り含む)
    #[error("更新ファイルのダウンロードに失敗しました")]
    DownloadFailed,
    /// 保存先への書き込み失敗
    #[error("更新ファイルの保存に失敗しました")]
    SaveFailed,
    /// サイズ不一致 (実測値が分かっている)
    #[error("更新ファイルのサイズが一致しません (期待 {expected} / 実際 {actual})")]
    SizeMismatch {
        /// 期待バイト数
        expected: u64,
        /// 実際のバイト数
        actual: u64,
    },
    /// サイズ上限超過で打ち切り (累積オーバーフロー)
    #[error("更新ファイルのサイズが上限を超えたためダウンロードを中断しました")]
    SizeCapOverflow,
    /// サイズ上限超過で打ち切り (実測が期待より大きい)
    #[error("更新ファイルのサイズが一致しません (期待 {expected} / 実際はそれ以上)")]
    SizeMismatchTruncated {
        /// 期待バイト数 (打ち切り上限)
        expected: u64,
    },
    /// SHA-256 不一致
    #[error("更新ファイルの完全性検証 (SHA-256) に失敗しました。再ダウンロードしてください")]
    IntegrityFailed,

    // ----------------------------------------------------------------------
    // 署名検証
    // ----------------------------------------------------------------------
    /// 署名検証用の公開鍵がビルドに埋め込まれていない
    #[error("このビルドには更新の署名検証用の公開鍵が含まれていないため、更新を適用できません")]
    SignatureKeyMissing,
    /// 署名ファイル取得の HTTP ステータスエラー
    #[error("更新ファイルの署名の取得に失敗しました (HTTP {status})")]
    SignatureDownloadHttp {
        /// HTTP ステータスコード
        status: u16,
    },
    /// 署名ファイル取得の一般失敗 (読み取り・サイズ超過含む)
    #[error("更新ファイルの署名の取得に失敗しました")]
    SignatureDownloadFailed,
    /// 署名の形式不正または検証失敗
    #[error("更新ファイルの署名検証に失敗しました。改ざんされている可能性があるため適用しません")]
    SignatureInvalid,
    /// 信頼する公開鍵以外で署名されている
    #[error("更新ファイルが信頼された鍵で署名されていないため適用しません")]
    SignatureUntrustedKey,
    /// 署名の信頼コメントが更新先バージョン・アセット名と一致しない
    #[error("更新ファイルの署名が更新先のバージョンまたはファイルと一致しないため適用しません")]
    SignatureMetadataMismatch,

    // ----------------------------------------------------------------------
    // 適用 (作業ディレクトリ / MSI / ZIP)
    // ----------------------------------------------------------------------
    /// 更新用一時フォルダの作成失敗
    #[error("更新用一時フォルダの作成に失敗しました")]
    WorkDirCreate,
    /// 未対応のアセット拡張子
    #[error("未対応のアセット形式です: {name}")]
    UnsupportedAsset {
        /// アセット名
        name: String,
    },
    /// `msiexec` 起動失敗
    #[error("インストーラーの起動に失敗しました")]
    MsiLaunch,
    /// 別のインストールが進行中 (`1618`)
    #[error("別のプログラムのインストールが進行中です。終わってから再実行してください")]
    MsiBusy,
    /// `msiexec` がその他の終了コードで失敗
    #[error("インストールに失敗しました (終了コード {code})")]
    MsiFailed {
        /// `msiexec` の終了コード
        code: i32,
    },
    /// インストーラーで導入されたフォルダではない
    #[error(
        "ClipRefiner がインストーラーの導入先とは別のフォルダで動作しているため更新できません。https://github.com/GnudMna/clip-refiner/releases から最新版を入手してください"
    )]
    InstallLocationMismatch,
    /// インストール後の実行ファイルが更新先の版ではない
    #[error("インストール後の実行ファイルが更新先の版になっていません")]
    InstalledVersionMismatch,
    /// 展開先の掃除失敗
    #[error("展開先フォルダの準備に失敗しました")]
    ExtractDirPrepare,
    /// 展開先の作成失敗
    #[error("展開先フォルダの作成に失敗しました")]
    ExtractDirCreate,
    /// ZIP オープン失敗
    #[error("更新 ZIP を開けませんでした")]
    ZipOpen,
    /// ZIP 形式不正
    #[error("更新 ZIP の形式が不正です")]
    ZipInvalid,
    /// ZIP 展開失敗
    #[error("更新 ZIP の展開に失敗しました")]
    ZipExtract,

    // ----------------------------------------------------------------------
    // 更新プログラムの起動 / アプリの終了と起動
    // ----------------------------------------------------------------------
    /// 更新プログラムが実行フォルダに無い
    #[error(
        "更新プログラム (ClipRefinerUpdater.exe) が見つかりません。https://github.com/GnudMna/clip-refiner/releases から最新版を入手してください"
    )]
    UpdaterMissing,
    /// 更新プログラムの起動失敗
    #[error("更新プログラムの起動に失敗しました")]
    UpdaterLaunch,
    /// 動作中のアプリへ終了を依頼できない
    #[error(
        "動作中の ClipRefiner へ終了を依頼できませんでした。更新の画面を閉じ、もう一度「更新を確認」を押してください"
    )]
    ExitRequest,
    /// アプリが終了しない
    #[error("ClipRefiner が終了しませんでした。タスクトレイから終了して再実行してください")]
    ExitTimeout,
    /// アプリの起動失敗
    #[error("ClipRefiner を起動できませんでした")]
    StartFailed,
    /// アプリが起動途中で終了した
    #[error("ClipRefiner が起動の途中で終了しました")]
    StartExited,
    /// アプリの起動完了を確認できない
    #[error("ClipRefiner の起動完了を確認できませんでした")]
    StartTimeout,
    /// 更新処理の内部エラー
    #[error("更新処理で内部エラーが発生しました")]
    Internal,

    // ----------------------------------------------------------------------
    // ポータブル差し替え
    // ----------------------------------------------------------------------
    /// 旧プロセス終了待ちタイムアウト
    #[error("旧プロセスの終了待ちがタイムアウトしました。手動で差し替えてください")]
    WaitPidTimeout,
    /// 旧プロセス終了待ちの一般失敗
    #[error("旧プロセスの終了待ちに失敗しました。手動で差し替えてください")]
    WaitPidFailed,
    /// 差し替え後の再起動失敗
    #[error("更新後の起動に失敗しました: {detail}")]
    RestartFailed {
        /// OS エラーなど詳細
        detail: String,
    },
    /// 展開先に exe が無い
    #[error("展開先に ClipRefiner.exe が見つかりません。手動で差し替えてください")]
    HelperExeMissing,
    /// 展開元フォルダが無い
    #[error("展開元フォルダが見つかりません")]
    SourceMissing,
    /// 差し替え先フォルダの準備失敗
    #[error("差し替え先フォルダの準備に失敗しました")]
    DestPrepare,
    /// 展開元の列挙失敗
    #[error("展開元フォルダを読めませんでした")]
    SourceRead,
    /// 個別ファイルの差し替え失敗
    #[error("ファイルの差し替えに失敗しました ({name})")]
    FileCopy {
        /// ファイル名
        name: String,
    },
    /// ファイル差し替えの一般失敗
    #[error("ファイルの差し替えに失敗しました")]
    FileReplace,
    /// フォルダ差し替えの一般失敗
    #[error("フォルダの差し替えに失敗しました")]
    DirReplace,
    /// 差し替えの失敗後に元の状態へ戻せなかった
    #[error(
        "ファイルの差し替えに失敗し、元の状態へ戻せませんでした。退避したファイルは {backup} にあります。手動で差し替えてください"
    )]
    RollbackFailed {
        /// 退避したファイルが残っているフォルダ
        backup: String,
    },
    /// 実行ファイルパスの取得失敗
    #[error("実行ファイルの場所を特定できませんでした")]
    ExePath,
    /// 実行ディレクトリの特定失敗
    #[error("実行ディレクトリを特定できませんでした")]
    ExeDir,
    /// 展開先と実行フォルダが同一
    #[error("展開先と実行フォルダが同じため差し替えできません")]
    SameReplaceDirs,
    /// 差し替え先が既存のポータブルフォルダではない
    #[error("差し替え先が ClipRefiner のポータブルフォルダではありません")]
    InstallDirInvalid,
    /// 展開元が更新作業フォルダの外にある
    #[error("展開元が更新用の一時フォルダではありません")]
    SourceDirInvalid,
    /// 削除対象が更新作業フォルダの外にある
    #[error("削除対象フォルダが更新用の一時フォルダではありません")]
    CleanupDirInvalid,
    /// `--wait-pid` が 0 またはヘルパー自身
    #[error("待ち受けプロセスの指定が不正です")]
    WaitPidInvalid,
    /// 生存中の待ち受けプロセスが `ClipRefiner` ではない
    #[error("待ち受けプロセスが ClipRefiner ではありません")]
    WaitPidNotClipRefiner,
    /// パスに不正な文字
    #[error("パスに不正な文字が含まれています")]
    PathInvalid,
}

impl UpdateError {
    /// 配布物種別の表示ラベルを返す
    ///
    /// # Arguments
    /// * `kind` - MSI / ポータブル
    ///
    /// # Returns
    /// * `&'static str` - UI / エラー文言用ラベル
    pub(crate) fn package_kind_label(kind: super::types::UpdatePackageKind) -> &'static str {
        match kind {
            super::types::UpdatePackageKind::Msi => "MSI",
            super::types::UpdatePackageKind::PortableZip => "ポータブル ZIP",
        }
    }
}
