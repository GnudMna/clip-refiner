//! 画像をエクスプローラへファイルとして貼り付けられるようにする
//!
//! PNG をディスクへ書き出さず、仮想ファイル (`FileGroupDescriptorW` / `FileContents`) として
//! OLE クリップボードへ載せる。OLE は貼り付けのたびに載せた側を呼び出すため、専用の STA スレッドが
//! メッセージを処理し続ける。スレッドが終わると仮想ファイルは消え、画像だけが残る

use std::io::{self, Cursor};
use std::mem::ManuallyDrop;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use clipboard_win::formats::CF_DIBV5;
use image::{ImageBuffer, ImageFormat, RgbaImage};
use windows::Win32::Foundation::{GlobalFree, HANDLE, WAIT_FAILED};
use windows::Win32::System::Com::{
    DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::GetClipboardOwner;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{OleInitialize, OleSetClipboard, OleUninitialize};
use windows::Win32::System::Threading::{CreateEventW, GetCurrentThreadId, INFINITE, SetEvent};
use windows::Win32::UI::Shell::SHCreateDataObject;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetWindowThreadProcessId, MSG, MWMO_INPUTAVAILABLE,
    MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
};

// ======================================================================
// 定数
// ======================================================================
/// 仮想ファイルの一覧を表す形式
const FILE_DESCRIPTOR_FORMAT: &str = "FileGroupDescriptorW";

/// 仮想ファイルの内容を表す形式
const FILE_CONTENTS_FORMAT: &str = "FileContents";

/// PNG 画像を表す形式
const PNG_FORMAT: &str = "PNG";

/// 貼り付けたときのファイル名 (拡張子を除く) の日時の書式
const FILE_STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";

/// クリップボードを開く最大試行回数
const OPEN_CLIPBOARD_ATTEMPTS: usize = 10;

/// クリップボードを開けなかったときに再試行するまでの間隔
const OPEN_CLIPBOARD_RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// `FILEDESCRIPTORW::cFileName` の文字数 (終端の NUL を含む)
const FILE_NAME_CHARS: usize = 260;

/// `FILEGROUPDESCRIPTORW` (1 件) のバイト数
const FILE_GROUP_DESCRIPTOR_LEN: usize = 596;

/// `BITMAPV5HEADER` のバイト数
const DIBV5_HEADER_LEN: u32 = 124;

// ======================================================================
// 仮想ファイルの提供スレッド
// ======================================================================
/// 画像と仮想ファイルをクリップボードへ載せ、貼り付けに応じ続けるスレッド
///
/// 破棄するとスレッドを止める。載せた画像は残り、仮想ファイルとしては貼り付けられなくなる
pub(crate) struct ImageFileHost {
    requests: Option<Sender<WriteRequest>>,
    wake: Arc<OwnedHandle>,
    join: Option<JoinHandle<()>>,
}

/// 提供スレッドへの書き込み要求
struct WriteRequest {
    content: ImageContent,
    reply: Sender<io::Result<()>>,
}

/// クリップボードへ載せる形に変換した画像
struct ImageContent {
    png: Vec<u8>,
    dib: Vec<u8>,
    descriptor: Vec<u8>,
}

impl ImageFileHost {
    /// 提供スレッドを起動し、OLE の初期化を待つ
    ///
    /// # Errors
    /// スレッドの起動、または OLE の初期化に失敗した場合
    pub(crate) fn start() -> io::Result<Self> {
        // SAFETY: 名前もセキュリティ属性も渡さず、戻り値のハンドルはこの関数が所有する
        let event = unsafe { CreateEventW(None, false, false, None) }.map_err(io::Error::other)?;
        // SAFETY: `event` は直前に作った有効なハンドルで、ほかに所有者がいない
        let wake = Arc::new(unsafe { OwnedHandle::from_raw_handle(event.0) });

        let (requests, request_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_wake = Arc::clone(&wake);
        let join = thread::Builder::new()
            .name("clip-refiner-image-file".into())
            .spawn(move || run_host_loop(&request_rx, &thread_wake, &ready_tx))?;

        ready_rx.recv().map_err(|_| host_stopped())??;
        Ok(Self {
            requests: Some(requests),
            wake,
            join: Some(join),
        })
    }

    /// RGBA 画像を、画像と PNG の仮想ファイルとしてクリップボードへ載せる
    ///
    /// 既存のクリップボード内容は消える。書き込みが終わるまで待つ
    ///
    /// # Errors
    /// バッファ長が寸法と合わない場合、またはクリップボードへの書き込みに失敗した場合
    pub(crate) fn write(&self, width: u32, height: u32, rgba: &[u8]) -> io::Result<()> {
        let content = ImageContent::new(width, height, rgba)?;
        let (reply, reply_rx) = mpsc::channel();
        self.requests
            .as_ref()
            .ok_or_else(host_stopped)?
            .send(WriteRequest { content, reply })
            .map_err(|_| host_stopped())?;
        self.wake()?;
        reply_rx.recv().map_err(|_| host_stopped())?
    }

    /// 提供スレッドを起こし、要求または切断を確認させる
    fn wake(&self) -> io::Result<()> {
        // SAFETY: `wake` は生存中のイベントのハンドルを指す
        unsafe { SetEvent(HANDLE(self.wake.as_raw_handle())) }.map_err(io::Error::other)
    }
}

impl Drop for ImageFileHost {
    fn drop(&mut self) {
        self.requests.take();
        // 起こせなければスレッドが終わらないため、終了を待たない
        if self.wake().is_ok()
            && let Some(join) = self.join.take()
        {
            let _ = join.join();
        }
    }
}

/// 提供スレッドの初期化済み OLE。破棄時に終了処理を行う
struct OleApartment;

impl OleApartment {
    fn initialize() -> io::Result<Self> {
        // SAFETY: 提供スレッドの開始時に 1 度だけ呼び、`Drop` の `OleUninitialize` と対にする
        unsafe { OleInitialize(None) }.map_err(io::Error::other)?;
        Ok(Self)
    }
}

impl Drop for OleApartment {
    fn drop(&mut self) {
        // SAFETY: `initialize` が成功したスレッドで、対になる 1 回だけ呼ぶ
        unsafe { OleUninitialize() };
    }
}

/// 要求の処理と、貼り付け先からの呼び出しに応じるメッセージ処理を続ける
fn run_host_loop(
    requests: &Receiver<WriteRequest>,
    wake: &OwnedHandle,
    ready: &Sender<io::Result<()>>,
) {
    let _ole = match OleApartment::initialize() {
        Ok(ole) => ole,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }

    loop {
        pump_messages();
        loop {
            match requests.try_recv() {
                Ok(WriteRequest { content, reply }) => {
                    let _ = reply.send(write_content(&content));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        // SAFETY: `wake` は呼び出し中に生存するイベントのハンドルを指す
        let waited = unsafe {
            MsgWaitForMultipleObjectsEx(
                Some(&[HANDLE(wake.as_raw_handle())]),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if waited == WAIT_FAILED {
            tracing::warn!("画像のファイル貼り付けの待機に失敗したため終了します");
            return;
        }
    }
}

/// 溜まっているウィンドウメッセージをすべて処理する
fn pump_messages() {
    let mut message = MSG::default();
    // SAFETY: `message` は呼び出し中に生存し、このスレッドのメッセージだけを取り出す
    while unsafe { PeekMessageW(&raw mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
        // SAFETY: `message` は `PeekMessageW` が書き込んだ有効なメッセージ
        unsafe {
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
}

/// 提供スレッドが終了している場合のエラー
fn host_stopped() -> io::Error {
    io::Error::other("画像のファイル貼り付けのスレッドが終了している")
}

// ======================================================================
// クリップボードへの書き込み
// ======================================================================
/// 画像と仮想ファイルをクリップボードへ載せる (提供スレッドで呼ぶ)
fn write_content(content: &ImageContent) -> io::Result<()> {
    let png_format = register_format(PNG_FORMAT)?;

    // SAFETY: フォルダも項目も渡さず、空のデータオブジェクトを作る
    let data: IDataObject =
        unsafe { SHCreateDataObject(None, None, None) }.map_err(io::Error::other)?;
    set_data(&data, png_format, -1, &content.png)?;
    set_data(&data, CF_DIBV5, -1, &content.dib)?;
    let descriptor_format = register_format(FILE_DESCRIPTOR_FORMAT)?;
    set_data(&data, descriptor_format, -1, &content.descriptor)?;
    // `lindex` は `FILEGROUPDESCRIPTORW` 内の位置を表す
    let contents_format = register_format(FILE_CONTENTS_FORMAT)?;
    set_data(&data, contents_format, 0, &content.png)?;

    set_ole_clipboard(&data)?;

    // OLE は要求時に内容を渡すため、アプリの終了後も画像を貼り付けられるよう実データで置き換える
    if let Err(e) = persist_image(png_format, content) {
        tracing::warn!("終了後も残る画像をクリップボードへ書き込めませんでした: {e}");
    }
    Ok(())
}

/// データオブジェクトを OLE クリップボードへ載せる
///
/// ほかのアプリがクリップボードを開いている間は、間隔を空けて再試行する
fn set_ole_clipboard(data: &IDataObject) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        // SAFETY: OLE を初期化した STA スレッドで呼び、`data` は呼び出し中に生存する
        match unsafe { OleSetClipboard(data) } {
            Ok(()) => return Ok(()),
            Err(e) if attempt >= OPEN_CLIPBOARD_ATTEMPTS => return Err(io::Error::other(e)),
            Err(_) => {
                attempt += 1;
                thread::sleep(OPEN_CLIPBOARD_RETRY_INTERVAL);
            }
        }
    }
}

/// 画像の形式を、提供スレッドの終了後も残る実データで置き換える
///
/// 載せた直後に別のコピーが入っていた場合は、その内容に触れない
fn persist_image(png_format: u32, content: &ImageContent) -> io::Result<()> {
    let _clipboard = clipboard_win::Clipboard::new_attempts(OPEN_CLIPBOARD_ATTEMPTS)
        .map_err(|e| io::Error::other(format!("クリップボードを開けない: {e}")))?;
    if !is_owned_by_current_thread() {
        return Ok(());
    }
    for (format, bytes) in [(png_format, &content.png), (CF_DIBV5, &content.dib)] {
        clipboard_win::raw::set_without_clear(format, bytes)
            .map_err(|e| io::Error::other(format!("画像を書き込めない: {e}")))?;
    }
    Ok(())
}

/// クリップボードの所有者が、このスレッドの OLE のウィンドウか
fn is_owned_by_current_thread() -> bool {
    // SAFETY: 引数はなく、所有者のウィンドウのハンドルを返すだけ
    let Ok(owner) = (unsafe { GetClipboardOwner() }) else {
        return false;
    };
    // SAFETY: `owner` が無効になっていても 0 を返すだけで、プロセス ID は受け取らない
    let owner_thread = unsafe { GetWindowThreadProcessId(owner, None) };
    // SAFETY: 引数はなく、呼び出し元のスレッド ID を返すだけ
    owner_thread == unsafe { GetCurrentThreadId() }
}

/// クリップボードの形式を登録し、その番号を返す
fn register_format(name: &str) -> io::Result<u32> {
    clipboard_win::register_format(name)
        .map(std::num::NonZeroU32::get)
        .ok_or_else(|| io::Error::other(format!("クリップボードの形式を登録できない: {name}")))
}

/// データオブジェクトへ、メモリ上のデータを 1 つの形式として追加する
fn set_data(data: &IDataObject, format: u32, lindex: i32, bytes: &[u8]) -> io::Result<()> {
    let format = FORMATETC {
        cfFormat: u16::try_from(format).map_err(io::Error::other)?,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex,
        tymed: TYMED_HGLOBAL.0.cast_unsigned(),
    };

    // SAFETY: 確保した領域は、成功時は `data`、失敗時はこの関数が解放する
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(io::Error::other)?;
    // SAFETY: `memory` は直前に確保した有効なハンドル
    let target = unsafe { GlobalLock(memory) };
    let result = if target.is_null() {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: `target` は `bytes.len()` バイト以上の領域を指し、`bytes` とは重ならない
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast(), bytes.len()) };
        // SAFETY: 直前の `GlobalLock` と対になる。ロックの解除後は `target` を参照しない
        let _ = unsafe { GlobalUnlock(memory) };
        let medium = STGMEDIUM {
            tymed: format.tymed,
            u: STGMEDIUM_0 { hGlobal: memory },
            pUnkForRelease: ManuallyDrop::new(None),
        };
        // SAFETY: `format` と `medium` は呼び出し中に生存し、成功時は `medium` の所有権が移る
        unsafe { data.SetData(&raw const format, &raw const medium, true) }
            .map_err(io::Error::other)
    };

    if result.is_err() {
        // SAFETY: 所有権が移っていないため、確保した領域をここで 1 度だけ解放する
        let _ = unsafe { GlobalFree(Some(memory)) };
    }
    result
}

// ======================================================================
// 形式の変換
// ======================================================================
impl ImageContent {
    /// RGBA 画像を、載せる形式それぞれへ変換する
    fn new(width: u32, height: u32, rgba: &[u8]) -> io::Result<Self> {
        let expected_len = u64::from(width) * u64::from(height) * 4;
        if u64::try_from(rgba.len()) != Ok(expected_len) {
            return Err(size_mismatch());
        }
        let png = encode_png(width, height, rgba)?;
        let stamp = jiff::ZonedDateTime::now().strftime(FILE_STAMP_FORMAT);
        Ok(Self {
            dib: encode_dibv5(width, height, rgba)?,
            descriptor: file_group_descriptor(&format!("{stamp}.png"), png.len())?,
            png,
        })
    }
}

/// RGBA 画像を PNG へエンコードする
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let image: RgbaImage =
        ImageBuffer::from_raw(width, height, rgba.to_vec()).ok_or_else(size_mismatch)?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(png)
}

/// RGBA 画像を `CF_DIBV5` (32bit・下の行から順) のバイト列にする
///
/// 上の行から順に並べる負の高さは Word が貼り付けられないため使わない
fn encode_dibv5(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    const PLANES_AND_BIT_COUNT: u32 = 1 | (32 << 16);
    const BI_BITFIELDS: u32 = 3;
    const LCS_SRGB: u32 = 0x7352_4742;
    const LCS_GM_IMAGES: u32 = 4;
    /// `CIEXYZTRIPLE` とガンマ値 3 つの `DWORD` 数 (`LCS_sRGB` では使われない)
    const ENDPOINTS_AND_GAMMA_DWORDS: usize = 12;

    let row_len = usize::try_from(width)
        .ok()
        .and_then(|w| w.checked_mul(4))
        .filter(|len| *len > 0)
        .ok_or_else(size_mismatch)?;
    let image_len = u32::try_from(rgba.len()).map_err(io::Error::other)?;
    let signed_width = i32::try_from(width).map_err(io::Error::other)?;
    let signed_height = i32::try_from(height).map_err(io::Error::other)?;

    let mut dib = Vec::with_capacity(DIBV5_HEADER_LEN as usize + rgba.len());
    dib.extend(DIBV5_HEADER_LEN.to_le_bytes());
    dib.extend(signed_width.to_le_bytes());
    dib.extend(signed_height.to_le_bytes());
    let fields = [
        PLANES_AND_BIT_COUNT,
        BI_BITFIELDS,
        image_len,
        0, // 横の解像度
        0, // 縦の解像度
        0, // 使用色数
        0, // 重要色数
        0x00ff_0000,
        0x0000_ff00,
        0x0000_00ff,
        0xff00_0000,
        LCS_SRGB,
    ];
    dib.extend(fields.iter().flat_map(|field| field.to_le_bytes()));
    dib.extend([0; ENDPOINTS_AND_GAMMA_DWORDS * 4]);
    dib.extend(LCS_GM_IMAGES.to_le_bytes());
    dib.extend([0; 3 * 4]); // プロファイルの位置・サイズと予約領域

    for row in rgba.rchunks(row_len) {
        dib.extend(
            row.as_chunks()
                .0
                .iter()
                .flat_map(|[r, g, b, a]| [*b, *g, *r, *a]),
        );
    }
    Ok(dib)
}

/// ファイル 1 件の `FILEGROUPDESCRIPTORW` のバイト列を作る
fn file_group_descriptor(name: &str, size: usize) -> io::Result<Vec<u8>> {
    const FD_ATTRIBUTES: u32 = 0x4;
    const FD_FILESIZE: u32 = 0x40;
    const FD_UNICODE: u32 = 0x8000_0000;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

    let mut name: Vec<u16> = name.encode_utf16().collect();
    if name.len() >= FILE_NAME_CHARS {
        return Err(io::Error::other("ファイル名が長すぎる"));
    }
    name.resize(FILE_NAME_CHARS, 0);
    let size = u32::try_from(size).map_err(io::Error::other)?;

    let mut descriptor = Vec::with_capacity(FILE_GROUP_DESCRIPTOR_LEN);
    descriptor.extend(1u32.to_le_bytes()); // 件数
    descriptor.extend((FD_ATTRIBUTES | FD_FILESIZE | FD_UNICODE).to_le_bytes());
    descriptor.extend([0; 16 + 8 + 8]); // CLSID・アイコンの大きさ・位置
    descriptor.extend(FILE_ATTRIBUTE_NORMAL.to_le_bytes());
    descriptor.extend([0; 3 * 8]); // 作成・アクセス・更新日時
    descriptor.extend(0u32.to_le_bytes()); // サイズの上位
    descriptor.extend(size.to_le_bytes());
    descriptor.extend(name.iter().flat_map(|c| c.to_le_bytes()));
    Ok(descriptor)
}

/// バッファ長が画像の寸法と合わない場合のエラー
fn size_mismatch() -> io::Error {
    io::Error::other("RGBA バッファの長さが画像の寸法と一致しない")
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 行と画素の並びを区別できる 2×2 の画像
    const RGBA_2X2: [u8; 16] = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];

    /// RGBA を PNG にエンコードでき、寸法の合わないバッファは拒否すること
    #[test]
    fn encode_png_roundtrips_and_rejects_bad_length() {
        let png = encode_png(2, 2, &RGBA_2X2).expect("encode");
        assert_eq!(&png[..4], b"\x89PNG");

        let decoded = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(decoded.into_raw(), RGBA_2X2);

        assert!(encode_png(2, 2, &[0; 4]).is_err());
    }

    /// `CF_DIBV5` を、ヘッダーの直後に下の行から順の BGRA が続く並びにすること
    #[test]
    fn encode_dibv5_matches_layout() {
        let dib = encode_dibv5(2, 2, &RGBA_2X2).expect("encode");
        let field = |offset: usize| {
            u32::from_le_bytes(dib[offset..offset + 4].try_into().expect("4 バイト"))
        };

        // サイズ・幅・高さ・BI_BITFIELDS・色ごとのマスク・色空間・用途
        assert_eq!((field(0), field(4), field(8)), (DIBV5_HEADER_LEN, 2, 2));
        assert_eq!(field(16), 3);
        assert_eq!(
            [field(40), field(44), field(48), field(52)],
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000]
        );
        assert_eq!((field(56), field(108)), (0x7352_4742, 4));

        assert_eq!(
            dib[DIBV5_HEADER_LEN as usize..],
            [9, 8, 7, 255, 12, 11, 10, 255, 3, 2, 1, 255, 6, 5, 4, 255]
        );
        assert!(encode_dibv5(0, 2, &[]).is_err());
    }

    /// 仮想ファイルの一覧に、件数・サイズ・NUL 終端の名前を規定の位置へ書くこと
    #[test]
    fn file_group_descriptor_matches_layout() {
        let descriptor = file_group_descriptor("a.png", 0x0102_0304).expect("descriptor");

        assert_eq!(descriptor.len(), FILE_GROUP_DESCRIPTOR_LEN);
        assert_eq!(descriptor[..4], 1u32.to_le_bytes());
        assert_eq!(descriptor[72..76], 0x0102_0304u32.to_le_bytes());
        let name: Vec<u8> = "a.png\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(descriptor[76..76 + name.len()], name);
    }

    /// 終端の NUL が収まらない長さの名前を拒否すること
    #[test]
    fn file_group_descriptor_rejects_long_name() {
        assert!(file_group_descriptor(&"a".repeat(FILE_NAME_CHARS - 1), 1).is_ok());
        assert!(file_group_descriptor(&"a".repeat(FILE_NAME_CHARS), 1).is_err());
    }

    /// 貼り付けるファイルの名前にアプリ名を含めず、日時だけにすること
    #[test]
    fn image_content_names_file_by_timestamp() {
        let content = ImageContent::new(2, 2, &RGBA_2X2).expect("content");
        let name: String = char::decode_utf16(
            content.descriptor[76..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|c| *c != 0),
        )
        .map(|c| c.expect("utf16"))
        .collect();

        let stem = name.strip_suffix(".png").expect("extension");
        assert!(jiff::civil::DateTime::strptime(FILE_STAMP_FORMAT, stem).is_ok());
    }

    /// クリップボードに触れずに、提供スレッドの起動と停止ができるようにすること
    #[test]
    fn host_starts_and_stops() {
        let host = ImageFileHost::start().expect("start");
        // 寸法の合わない画像は、短くても長くてもスレッドへ渡す前に拒否する
        assert!(host.write(2, 2, &[0; 4]).is_err());
        assert!(host.write(2, 2, &[0; 20]).is_err());
        drop(host);
    }

    /// 実クリップボードへ画像と仮想ファイルを載せ、停止後は画像だけを残すこと
    ///
    /// 実クリップボードの内容を書き換えるため、手動実行 (`cargo test -- --ignored`) 専用
    #[test]
    #[ignore = "実クリップボードを書き換える"]
    fn write_puts_image_and_virtual_file_on_clipboard() {
        use clipboard_win::is_format_avail;

        let descriptor = register_format(FILE_DESCRIPTOR_FORMAT).expect("format");
        let png = register_format(PNG_FORMAT).expect("format");
        let host = ImageFileHost::start().expect("start");
        host.write(2, 2, &RGBA_2X2).expect("write");

        assert!(is_format_avail(CF_DIBV5), "ビットマップが無い");
        assert!(is_format_avail(png), "PNG が無い");
        assert!(is_format_avail(descriptor), "仮想ファイルが無い");
        assert!(
            !is_format_avail(clipboard_win::formats::CF_HDROP),
            "実ファイルのパスが載っている"
        );

        drop(host);
        assert!(is_format_avail(CF_DIBV5), "ビットマップが残っていない");
        assert!(is_format_avail(png), "PNG が残っていない");
        assert!(!is_format_avail(descriptor), "仮想ファイルが残っている");
    }
}
