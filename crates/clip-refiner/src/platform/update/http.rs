//! 更新用 HTTP クライアント (`ureq::Agent`) の生成
//!
//! `ureq` は既定でタイムアウトを持たないため、接続や応答が止まると更新確認・ダウンロードの
//! スレッドが戻らず UI が「確認中」のまま残る。呼び出し側は必ずここの Agent を使う

use std::time::Duration;

use ureq::Agent;

// ======================================================================
// 定数
// ======================================================================
/// 接続確立 (DNS 解決後のソケット接続と TLS ハンドシェイク) の上限
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 更新確認・署名取得の 1 リクエスト全体 (接続から本文の受信完了まで) の上限
const API_GLOBAL_TIMEOUT: Duration = Duration::from_secs(30);
/// ダウンロードで応答ヘッダを待つ上限
const DOWNLOAD_RECV_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// ダウンロード本文の受信全体の上限
///
/// 読み取りごとではなく本文全体の予算になる。絶対サイズ上限 (512 MiB) を
/// 約 0.3 MiB/s の低速回線でも受け切れる長さにする
const DOWNLOAD_RECV_BODY_TIMEOUT: Duration = Duration::from_mins(30);

// ======================================================================
// パブリック関数
// ======================================================================
/// 更新確認・署名取得用の Agent を返す
///
/// 小さな応答を 1 往復で受ける用途のため、リクエスト全体に短い上限を課す
///
/// # Returns
/// * `Agent` - 接続とリクエスト全体にタイムアウトを設定した Agent
pub(super) fn api_agent() -> Agent {
    api_agent_with_global_timeout(API_GLOBAL_TIMEOUT)
}

/// 更新アセットのダウンロード用 Agent を返す
///
/// 本文は大きいためリクエスト全体の上限は置かず、接続・応答ヘッダ・本文受信にそれぞれ上限を課す
///
/// # Returns
/// * `Agent` - 接続・応答ヘッダ・本文受信にタイムアウトを設定した Agent
pub(super) fn download_agent() -> Agent {
    download_agent_with_timeouts(DOWNLOAD_RECV_RESPONSE_TIMEOUT, DOWNLOAD_RECV_BODY_TIMEOUT)
}

// ======================================================================
// プライベート関数
// ======================================================================
/// リクエスト全体の上限を指定して更新確認用 Agent を組み立てる
fn api_agent_with_global_timeout(global: Duration) -> Agent {
    Agent::new_with_config(
        Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(global))
            .build(),
    )
}

/// 応答ヘッダと本文受信の上限を指定してダウンロード用 Agent を組み立てる
fn download_agent_with_timeouts(recv_response: Duration, recv_body: Duration) -> Agent {
    Agent::new_with_config(
        Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(recv_response))
            .timeout_recv_body(Some(recv_body))
            .build(),
    )
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{api_agent_with_global_timeout, download_agent_with_timeouts};

    /// 接続を受けても応答を返さないローカルサーバを起動し、その URL を返す
    ///
    /// 受けた接続は `hold` の間だけ開いたままにして閉じる
    fn spawn_silent_server(hold: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/", listener.local_addr().expect("addr"));
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                thread::sleep(hold);
            }
        });
        url
    }

    /// 応答しないサーバに対して更新確認用 Agent が上限内でエラーを返すこと
    #[test]
    fn api_agent_times_out_when_server_stays_silent() {
        let url = spawn_silent_server(Duration::from_secs(5));
        let agent = api_agent_with_global_timeout(Duration::from_millis(300));

        let started = Instant::now();
        let result = agent.get(&url).call();

        assert!(matches!(result, Err(ureq::Error::Timeout(_))));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    /// 応答ヘッダが来ないサーバに対してダウンロード用 Agent が上限内でエラーを返すこと
    #[test]
    fn download_agent_times_out_waiting_for_response_headers() {
        let url = spawn_silent_server(Duration::from_secs(5));
        let agent =
            download_agent_with_timeouts(Duration::from_millis(300), Duration::from_mins(1));

        let started = Instant::now();
        let result = agent.get(&url).call();

        assert!(matches!(result, Err(ureq::Error::Timeout(_))));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    /// 本文の途中で止まったサーバに対してダウンロード用 Agent が本文受信の上限で打ち切ること
    #[test]
    fn download_agent_times_out_while_receiving_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/", listener.local_addr().expect("addr"));
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                // 宣言した長さより少ない本文だけ送り、その後は接続を保持して止まる
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\nabc");
                let _ = stream.flush();
                thread::sleep(Duration::from_secs(5));
            }
        });
        let agent =
            download_agent_with_timeouts(Duration::from_secs(5), Duration::from_millis(300));

        let started = Instant::now();
        let mut response = agent.get(&url).call().expect("ヘッダは受信できる");
        let mut sink = Vec::new();
        let read = response.body_mut().as_reader().read_to_end(&mut sink);

        assert!(read.is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
