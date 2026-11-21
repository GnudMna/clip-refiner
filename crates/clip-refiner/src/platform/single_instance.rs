//! 名前付き mutex による多重起動の判定
//!
//! 同名の mutex が既に存在するかで、他のプロセスが同じ名前を保持しているかを調べる

use anyhow::{Context, Result};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::{HSTRING, Owned};

// ======================================================================
// 型
// ======================================================================
/// 名前付き mutex の保持状態
///
/// 最初に作成したインスタンスだけが mutex を保持し、破棄時に解放する
pub struct SingleInstance {
    /// 自分が作成した mutex (既に存在していた場合は `None`)
    handle: Option<Owned<windows::Win32::Foundation::HANDLE>>,
}

impl SingleInstance {
    /// 指定名の mutex を作成し、他のプロセスが保持していないかを調べる
    ///
    /// # Arguments
    /// * `name` - mutex 名
    ///
    /// # Returns
    /// * `Result<SingleInstance>` - 作成または既存の確認ができた場合のインスタンス
    ///
    /// # Errors
    /// `CreateMutexW` が失敗した場合
    pub fn new(name: &str) -> Result<Self> {
        let name = HSTRING::from(name);
        // SAFETY: `name` は NUL 終端の UTF-16 で、呼び出し完了まで生存する
        let created =
            unsafe { CreateMutexW(None, false, &name) }.context("名前付き mutex の作成に失敗")?;
        // SAFETY: 直前の API 呼び出しのエラーコードを読むだけで、間に他の Win32 呼び出しを挟まない
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        // SAFETY: `CreateMutexW` が成功して返したハンドルで、ここで唯一の所有者になる
        let handle = unsafe { Owned::new(created) };

        // 既存の mutex を開いただけの場合は保持せず、すぐ閉じる
        Ok(Self {
            handle: (!already_exists).then_some(handle),
        })
    }

    /// 他のプロセスが同名の mutex を保持していないか
    ///
    /// # Returns
    /// * `bool` - 自分だけの場合は `true`
    pub fn is_single(&self) -> bool {
        self.handle.is_some()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::SingleInstance;

    /// 同名の 2 つ目は単独ではなく、先のインスタンスを破棄すると再び単独になること
    #[test]
    fn second_instance_with_same_name_is_not_single() {
        let name = format!("clip-refiner-test-single-instance-{}", std::process::id());

        let first = SingleInstance::new(&name).expect("first");
        assert!(first.is_single());

        // 保持中は同名の 2 つ目が単独にならない
        let second = SingleInstance::new(&name).expect("second");
        assert!(!second.is_single());

        // 解放後は再び取得できる
        drop(first);
        drop(second);
        let third = SingleInstance::new(&name).expect("third");
        assert!(third.is_single());
    }

    /// 名前が異なれば互いに単独として扱うこと
    #[test]
    fn different_names_are_independent() {
        let base = format!("clip-refiner-test-independent-{}", std::process::id());

        let a = SingleInstance::new(&format!("{base}-a")).expect("a");
        let b = SingleInstance::new(&format!("{base}-b")).expect("b");

        assert!(a.is_single());
        assert!(b.is_single());
    }
}
