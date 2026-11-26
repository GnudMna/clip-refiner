//! アプリ更新フローの統合テスト用ヘルパー
//!
//! `cargo test` 実行時、または `test-helpers` feature 有効時に利用可能

#![allow(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "テスト専用ヘルパーで、失敗時の panic は仕様"
)]

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::check::check_for_update_at;
use crate::prepare::prepare_update_at;
use crate::signature::{expected_trusted_comment, signature_asset_name};
use crate::{
    AvailableUpdate, PreparedPackage, RELEASE_OWNER, RELEASE_REPO, UpdateCheckOutcome, UpdateError,
    UpdatePackageKind,
};

// ======================================================================
// 定数
// ======================================================================
/// SHA-256 16進変換用文字表
const HEX: &[u8; 16] = b"0123456789abcdef";
/// アセット本体を返すパスの接頭辞 (ダウンロード URL のリダイレクト先)
const ASSET_STORAGE_PREFIX: &str = "/asset-storage";

// ======================================================================
// モックサーバ
// ======================================================================
/// ローカル HTTP で GitHub API と GitHub Releases の互換パスを返すモックサーバ
///
/// アセットは GitHub と同様に、別パスへのリダイレクトを経由して返す
pub struct MockReleaseServer {
    /// `http://127.0.0.1:{port}` 形式のベース URL
    base_url: String,
    routes: Arc<Mutex<ReleaseRoutes>>,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

/// モックが返すリリース JSON とアセット本文
#[derive(Debug, Default, Clone)]
struct ReleaseRoutes {
    /// `GET /repos/{owner}/{repo}/releases/latest` のステータスと本文 (`None` なら 404)
    latest_release: Option<(u16, Vec<u8>)>,
    /// パス (`/{owner}/{repo}/releases/download/{tag}/{asset}`) → 本文
    assets: HashMap<String, Vec<u8>>,
}

impl MockReleaseServer {
    /// 未設定ルートでモックサーバを起動する
    ///
    /// # Returns
    /// * `Self` - 待受中のモック
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        listener
            .set_nonblocking(true)
            .expect("nonblocking mock server");
        let port = listener.local_addr().expect("local addr").port();
        let base_url = format!("http://127.0.0.1:{port}");
        let routes = Arc::new(Mutex::new(ReleaseRoutes::default()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let routes_thread = Arc::clone(&routes);
        let shutdown_thread = Arc::clone(&shutdown);
        let join = thread::spawn(move || {
            while !shutdown_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // listener が nonblocking だと accept 済みソケットも継承される
                        // リクエスト到着前の WouldBlock で空応答→404 にしないよう blocking へ戻す
                        if let Err(err) = stream.set_nonblocking(false) {
                            tracing::warn!("モックサーバの blocking 設定失敗: {err}");
                            continue;
                        }
                        if let Err(err) = handle_client(&mut stream, &routes_thread) {
                            tracing::warn!("モックサーバの応答失敗: {err}");
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            base_url,
            routes,
            shutdown,
            join: Some(join),
        }
    }

    /// ベース URL を返す
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `&str` - `http://127.0.0.1:{port}` 形式のベース URL
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 最新リリースの JSON 本文を設定する
    ///
    /// # Arguments
    /// * `body` - レスポンス本文
    pub fn set_release_json(&self, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latest_release = Some((200, body.into()));
    }

    /// 最新リリースの照会を指定ステータスのエラーにする
    ///
    /// # Arguments
    /// * `status` - HTTP ステータスコード
    pub fn set_release_error(&self, status: u16) {
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latest_release = Some((status, Vec::new()));
    }

    /// アセットダウンロードパスへ本文を登録する
    ///
    /// # Arguments
    /// * `tag` - リリースタグ
    /// * `asset_name` - アセット名
    /// * `body` - ファイル本文
    pub fn set_asset(&self, tag: &str, asset_name: &str, body: impl Into<Vec<u8>>) {
        let path = format!("/{RELEASE_OWNER}/{RELEASE_REPO}/releases/download/{tag}/{asset_name}");
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .assets
            .insert(path, body.into());
    }
}

impl Drop for MockReleaseServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // accept 待ちを起こす
        if let Ok(stream) = TcpStream::connect(self.base_url.trim_start_matches("http://")) {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

// ======================================================================
// 更新 API ラッパ
// ======================================================================
/// モックサーバ向けに更新確認する
///
/// # Arguments
/// * `base_url` - モックのベース URL
/// * `current_raw` - 現在バージョン
/// * `kind` - MSI / ポータブル
/// * `arch` - CPU アーキテクチャ
///
/// # Returns
/// * `UpdateCheckOutcome` - 確認結果
pub fn check_update_at(
    base_url: &str,
    current_raw: &str,
    kind: UpdatePackageKind,
    arch: &str,
) -> UpdateCheckOutcome {
    check_for_update_at(base_url, current_raw, kind, arch)
}

/// モックサーバ向けに更新のダウンロード・検証・展開まで行う
///
/// # Arguments
/// * `update` - 確認済み更新
/// * `base_url` - モックのベース URL
/// * `trusted_keys` - 署名検証に使う minisign 公開鍵 (Base64)
///
/// # Returns
/// * `Result<PreparedPackage, UpdateError>` - インストールできる状態の配布物
///
/// # Errors
/// ダウンロード・検証・展開のいずれかが失敗した場合
pub fn prepare_update_for_test(
    update: &AvailableUpdate,
    base_url: &str,
    trusted_keys: &[&str],
) -> Result<PreparedPackage, UpdateError> {
    prepare_update_at(update, base_url, trusted_keys, &mut |_| {})
}

/// 更新署名の信頼コメントとして期待する文字列を返す
///
/// # Arguments
/// * `version` - 更新先バージョン (`SemVer`、`v` なし)
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - リリース時に署名へ入れる信頼コメント
#[must_use]
pub fn update_signature_trusted_comment(version: &str, asset_name: &str) -> String {
    expected_trusted_comment(version, asset_name)
}

/// アセットに対応する署名ファイル名を返す
///
/// # Arguments
/// * `asset_name` - アセット名
///
/// # Returns
/// * `String` - `<asset_name>.minisig`
#[must_use]
pub fn update_signature_asset_name(asset_name: &str) -> String {
    signature_asset_name(asset_name)
}

/// ポータブル ZIP フィクスチャを組み立てる
///
/// 展開後に `pkg/PORTABLE.txt` が現れる単一フォルダ構成
///
/// # Arguments
/// * `marker` - `PORTABLE.txt` の内容
///
/// # Returns
/// * `(Vec<u8>, String, u64)` - ZIP 本文 / SHA-256 16進 / バイト数
pub fn build_portable_zip_fixture(marker: &[u8]) -> (Vec<u8>, String, u64) {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut cursor);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("pkg/PORTABLE.txt", options)
            .expect("zip start");
        writer.write_all(marker).expect("zip write");
        writer.finish().expect("zip finish");
    }
    let bytes = cursor.into_inner();
    let digest = sha256_hex(&bytes);
    let size = bytes.len() as u64;
    (bytes, digest, size)
}

/// バイト列の SHA-256 を小文字16進で返す
///
/// # Arguments
/// * `bytes` - 入力
///
/// # Returns
/// * `String` - 64 桁の16進
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for &byte in digest.as_slice() {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// 最新リリースの JSON を組み立てる
///
/// # Arguments
/// * `tag` - リリースタグ
/// * `asset_name` - アセット名
/// * `size` - バイトサイズ
/// * `digest` - `sha256:<hex>` (`None` で欠落)
///
/// # Returns
/// * `String` - JSON 本文
pub fn release_api_json(tag: &str, asset_name: &str, size: u64, digest: Option<&str>) -> String {
    let digest_field = match digest {
        Some(value) => format!(r#","digest":"{value}""#),
        None => String::new(),
    };
    format!(
        r#"{{"tag_name":"{tag}","assets":[{{"name":"{asset_name}","size":{size}{digest_field}}}]}}"#
    )
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 1 クライアント分の HTTP リクエストを処理する
fn handle_client(stream: &mut TcpStream, routes: &Mutex<ReleaseRoutes>) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if buffer.len() > 64 * 1024 {
                    break;
                }
            }
            // blocking + read_timeout 下では TimedOut。ヘッダ未完了なら打ち切る
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(err) => return Err(err),
        }
    }

    // shutdown 起床用の空接続など、要求行が無い場合は応答しない
    if buffer.is_empty() {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer);
    let path = request
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            let method = parts.next()?;
            let path = parts.next()?;
            if method == "GET" {
                Some(path.split('?').next().unwrap_or(path).to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    if path.is_empty() {
        return Ok(());
    }

    let release_path = format!("/repos/{RELEASE_OWNER}/{RELEASE_REPO}/releases/latest");
    let response = {
        let guard = routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if path == release_path {
            match &guard.latest_release {
                Some((status, body)) => http_response(*status, "application/json", body),
                None => http_response(404, "text/plain", b"not found"),
            }
        } else if guard.assets.contains_key(&path) {
            redirect_response(&format!("{ASSET_STORAGE_PREFIX}{path}"))
        } else if let Some(body) = path
            .strip_prefix(ASSET_STORAGE_PREFIX)
            .and_then(|asset_path| guard.assets.get(asset_path))
        {
            http_response(200, "application/octet-stream", body)
        } else {
            http_response(404, "text/plain", b"not found")
        }
    };

    stream.write_all(&response)?;
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

/// 最小限の HTTP/1.1 レスポンスを組み立てる
fn http_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut out = header.into_bytes();
    out.extend_from_slice(body);
    out
}

/// 指定パスへの 302 リダイレクトを組み立てる
fn redirect_response(location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}
