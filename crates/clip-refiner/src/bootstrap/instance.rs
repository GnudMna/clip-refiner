//! トレイ常駐の多重起動防止

use anyhow::{Context, Result};

use crate::consts;
use crate::log_warn;
use crate::platform;
use crate::platform::single_instance::SingleInstance;

// ======================================================================
// 多重起動防止
// ======================================================================
/// トレイ常駐の多重起動を防止し、インスタンスを保持する
///
/// ワンショット実行では呼ばない。通常の常駐起動では `APP_ID` を使う。
/// E2E 起動ではプロセス固有 ID にし、常駐インスタンスと衝突しない
///
/// # Arguments
/// * `e2e` - E2E モードなら `true`
///
/// # Returns
/// * `Result<SingleInstance>` - シングルインスタンスであることが確認できた場合、そのインスタンスを返す
///   既に他の常駐インスタンスが実行中の場合は、通知を表示してプロセスを直ちに終了する
pub(super) fn ensure_single_instance(e2e: bool) -> Result<SingleInstance> {
    let instance_id = if e2e {
        format!("{}-e2e-{}", consts::APP_ID, std::process::id())
    } else {
        consts::APP_ID.to_string()
    };
    let instance = SingleInstance::new(&instance_id)
        .context("多重起動防止のインスタンス作成に失敗しました")?;

    if !instance.is_single() {
        let msg = format!("{}は既に実行されています。", consts::APP_NAME);
        log_warn!("{}", msg);
        platform::show_notification("多重起動", &msg);
        // 多重起動時は即座に終了するため、ErrではなくOkの扱いにしつつメッセージを表示
        std::process::exit(0);
    }

    Ok(instance)
}
