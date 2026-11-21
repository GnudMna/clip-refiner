//! アプリ更新フローの統合テスト用ヘルパー

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::platform::update::{
    RELEASE_OWNER, RELEASE_REPO, apply_update_at, apply_update_at_with_key, check_for_update_at,
};

use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

// ======================================================================
// 定数
// ======================================================================
/// SHA-256 16進変換用文字表
const HEX: &[u8; 16] = b"0123456789abcdef";

// ======================================================================
// 再エクスポート
// ======================================================================
pub use crate::platform::update::{
    AvailableUpdate, UpdateApplyOutcome, UpdateCheckOutcome, UpdatePackageKind,
};

// ======================================================================
// モック bridge
// ======================================================================
/// ローカル HTTP で gh-release-bridge 互換パスを返すモックサーバ
pub struct MockReleaseBridge {
    /// `http://127.0.0.1:{port}` 形式のベース URL
    base_url: String,
    routes: Arc<Mutex<BridgeRoutes>>,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

/// モックが返す version JSON とアセット本文
#[derive(Debug, Default, Clone)]
struct BridgeRoutes {
    /// `GET /{owner}/{repo}/version` の本文 (空なら 404)
    version_body: Option<Vec<u8>>,
    /// パス (`/{owner}/{repo}/{tag}/{asset}`) → 本文
    assets: HashMap<String, Vec<u8>>,
    /// アセット DL に要求する Bearer キー (`None` ならキー不要)
    required_asset_key: Option<String>,
}

impl MockReleaseBridge {
    /// 未設定ルートでモック bridge を起動する
    ///
    /// # Returns
    /// * `Self` - 待受中のモック
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock bridge");
        listener
            .set_nonblocking(true)
            .expect("nonblocking mock bridge");
        let port = listener.local_addr().expect("local addr").port();
        let base_url = format!("http://127.0.0.1:{port}");
        let routes = Arc::new(Mutex::new(BridgeRoutes::default()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let routes_thread = Arc::clone(&routes);
        let shutdown_thread = Arc::clone(&shutdown);
        let join = thread::spawn(move || {
            while !shutdown_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(err) = handle_client(stream, &routes_thread) {
                            crate::log_warn!("モック bridge 応答失敗: {err}");
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

    /// bridge ベース URL を返す
    ///
    /// # Returns
    /// * `&str` - `http://127.0.0.1:{port}`
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// version API の JSON 本文を設定する
    ///
    /// # Arguments
    /// * `body` - レスポンス本文
    pub fn set_version_json(&self, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .version_body = Some(body.into());
    }

    /// version API を 404 にする
    pub fn clear_version_json(&self) {
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .version_body = None;
    }

    /// アセットダウンロードパスへ本文を登録する
    ///
    /// # Arguments
    /// * `tag` - リリースタグ
    /// * `asset_name` - アセット名
    /// * `body` - ファイル本文
    pub fn set_asset(&self, tag: &str, asset_name: &str, body: impl Into<Vec<u8>>) {
        let path = format!("/{RELEASE_OWNER}/{RELEASE_REPO}/{tag}/{asset_name}");
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .assets
            .insert(path, body.into());
    }

    /// アセット DL に要求する Bearer キーを設定する
    ///
    /// version API はキー不要のまま。`None` 相当の空文字はキー不要へ戻す
    ///
    /// # Arguments
    /// * `key` - 期待する共有キー
    pub fn set_required_asset_key(&self, key: impl Into<String>) {
        let key = key.into();
        self.routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .required_asset_key = if key.is_empty() { None } else { Some(key) };
    }
}

impl Drop for MockReleaseBridge {
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
/// モック bridge 向けに更新確認する
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

/// モック bridge 向けに更新を適用する (外部プロセスは起動しない)
///
/// ダウンロードキーはビルド埋め込み値を使う
///
/// # Arguments
/// * `update` - 確認済み更新
/// * `base_url` - モックのベース URL
///
/// # Returns
/// * `UpdateApplyOutcome` - 適用結果
pub fn apply_update_for_test(update: &AvailableUpdate, base_url: &str) -> UpdateApplyOutcome {
    apply_update_at(
        update,
        base_url,
        crate::platform::update::ApplyOptions::TEST,
    )
}

/// 指定キーでモック bridge 向けに更新を適用する (外部プロセスは起動しない)
///
/// # Arguments
/// * `update` - 確認済み更新
/// * `base_url` - モックのベース URL
/// * `download_key` - bridge ダウンロードキー (`None` なら無認証)
///
/// # Returns
/// * `UpdateApplyOutcome` - 適用結果
pub fn apply_update_for_test_with_key(
    update: &AvailableUpdate,
    base_url: &str,
    download_key: Option<&str>,
) -> UpdateApplyOutcome {
    apply_update_at_with_key(
        update,
        base_url,
        crate::platform::update::ApplyOptions::TEST,
        download_key,
    )
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

/// version API JSON を組み立てる
///
/// # Arguments
/// * `tag` - リリースタグ
/// * `asset_name` - アセット名
/// * `size` - バイトサイズ
/// * `digest` - `sha256:<hex>` または素の16進 (`None` で欠落)
///
/// # Returns
/// * `String` - JSON 本文
pub fn version_api_json(tag: &str, asset_name: &str, size: u64, digest: Option<&str>) -> String {
    let digest_field = match digest {
        Some(value) => format!(r#","digest":"{value}""#),
        None => String::new(),
    };
    format!(r#"{{"tag":"{tag}","assets":[{{"name":"{asset_name}","size":{size}{digest_field}}}]}}"#)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 1 クライアント分の HTTP リクエストを処理する
fn handle_client(mut stream: TcpStream, routes: &Mutex<BridgeRoutes>) -> std::io::Result<()> {
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
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => return Err(err),
        }
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
    let bearer = extract_bearer(&request);

    let version_path = format!("/{RELEASE_OWNER}/{RELEASE_REPO}/version");
    let response = {
        let guard = routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if path == version_path {
            // version API はキー不要
            match &guard.version_body {
                Some(body) => http_response(200, "application/json", body),
                None => http_response(404, "text/plain", b"not found"),
            }
        } else if let Some(body) = guard.assets.get(&path) {
            if let Some(required) = guard.required_asset_key.as_deref() {
                if bearer == Some(required) {
                    http_response(200, "application/octet-stream", body)
                } else {
                    http_response(401, "text/plain", b"unauthorized")
                }
            } else {
                http_response(200, "application/octet-stream", body)
            }
        } else {
            http_response(404, "text/plain", b"not found")
        }
    };

    stream.write_all(&response)?;
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

/// リクエストから `Authorization: Bearer` のトークンを取り出す
fn extract_bearer(request: &str) -> Option<&str> {
    for line in request.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("Authorization:")
            .or_else(|| trimmed.strip_prefix("authorization:"))
        else {
            continue;
        };
        let rest = rest.trim();
        let Some(token) = rest
            .strip_prefix("Bearer ")
            .or_else(|| rest.strip_prefix("bearer "))
            .or_else(|| rest.strip_prefix("BEARER "))
        else {
            continue;
        };
        let token = token.trim();
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

/// 最小限の HTTP/1.1 レスポンスを組み立てる
fn http_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
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
