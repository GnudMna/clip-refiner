# テストと品質チェック

テストの構成と実行方法、品質チェック一式 (`check.ps1`) をまとめる。環境の準備は [setup.md](setup.md) を参照。

## テスト

変更中は対象テストを絞り、完了前に [品質チェック](#品質チェック) 一式 (`check.ps1`) を実行する。

```powershell
# 単体テスト・統合テスト (check.ps1 と同じ条件。--release を付けても通る)
cargo test --all-targets --features test-helpers

# 特定の統合テストのみ実行 (短いフィードバックループ)
cargo test --test monitor_flow
cargo test --test settings_flow

# UI 純粋ロジック (Node.js。未インストール時はスキップ)
cargo test --test settings_shared_logic
cargo test --test selector_logic

# core をテキスト変換のみ (feature なし) で検証
cargo test -p clip-refiner-core --no-default-features --all-targets

# ライブラリ API のドキュメントを生成
cargo doc -p clip-refiner-core --no-deps --open
```

UI の JavaScript (`crates/clip-refiner/src/ui/`) を変更したら `npm run build:ui` でバンドルを再生成し、`*.bundle.js` も同じコミットに含める。再生成を忘れると `cargo test` の `ui_bundle_flow` が失敗する ([JavaScript リンター](#javascript-リンター-eslint--prettier))。

統合テスト (`crates/clip-refiner/tests/`) では監視ループ・ワーカー・履歴・正規表現・連続記録・登録クリップの主要経路を検証する。`ClipboardHarness` が監視ループとワーカーコマンドを横断的に駆動する。

| ファイル | 検証内容 |
| :--- | :--- |
| `monitor_flow.rs` | 監視加工、Undo、自動変換無効時のスキップ、履歴上限、本文上限超過時の非加工、同一テキストの再加工抑制、自身の書き戻しスキップ、パイプライン、連続記録中の自動加工停止、HTML 形式の読み取り (`html-to-markdown`) と書き込み (`markdown-to-rich-text`) |
| `worker_flow.rs` | 手動加工 (`ProcessMode`)、変更なし加工、Undo 空実行、履歴復元後の再加工抑制 (Polling / Event) |
| `worker_recovery_flow.rs` | クリップボードワーカーの起動、`restart` による復旧、復旧メニュー表示 |
| `history_flow.rs` | 暗号化履歴の記録・重複処理・復号 |
| `regex_flow.rs` | `config` の正規表現設定、コンパイルキャッシュ、Event 方式 |
| `ocr_flow.rs` | OCR 結果のクリップボード書き込みと再加工抑制 |
| `record_flow.rs` | 連続記録キュー (FIFO、貼り付け排出、履歴との独立性) |
| `clips_flow.rs` | 登録クリップの保存・コピーと再加工抑制、本文・ラベルの編集後のコピー、テンプレートの展開 (通常クリップは展開しない・書式不正時はクリップボードを変えない) |
| `settings_flow.rs` | 設定ウィンドウ IPC のマージ・パッチ適用・監視/正規表現/連続記録/自動変換と連続記録の排他・`config.toml` 永続化往復 |
| `notification_flow.rs` | 通知設定の旧項目移行・IPC パッチ・トレイメニュー連携 |
| `platform_flow.rs` | ホットキー解釈・OS 登録・自動起動状態照会 |
| `update_flow.rs` | ローカル HTTP モック bridge による更新確認 → ダウンロード検証 → ポータブル ZIP 展開 / MSI 検証 (digest 欠落・ハッシュ / サイズ不一致・署名の欠落 / 信頼しない鍵 / 内容差し替え / 別バージョン向け署名 / 公開鍵なし・応答本文の宣言サイズ超過による途中打ち切り・API 404・キー必須 DL の成功 / 認証失敗含む。`msiexec` / 差し替えヘルパー / エクスプローラーは起動しない) |
| `regex_preview_logic.rs` | 設定 UI の正規表現プレビュー JS ロジック (Node.js 実行) |
| `settings_ui_logic.rs` | 設定 UI のパッチマージ JS ロジック (`settings_patch.js` / Node.js 実行) |
| `settings_shared_logic.rs` | 設定 UI 共有純粋ロジック (`shared_logic.js` / Node.js 実行) |
| `settings_panels_logic.rs` | 設定 UI パネル / イベント純粋ロジック (`panels_logic.js`: サイドバー HTML・WAI-ARIA タブ移動・クリップ行・パイプライン操作・クリック委譲 / Node.js 実行) |
| `selector_logic.rs` | セレクター共有・クイック・登録クリップ純粋ロジック (`selector-common.js` / `quick_selector_logic.js` / `clip_selector_logic.js` / Node.js 実行) |
| `ui_bundle_flow.rs` | コミット済み `*.bundle.js` 先頭の入力一覧・SHA-256 をソースから再計算し、バンドルの再生成忘れを検出 (Node.js 不要) |
| `license_ui_flow.rs` | 設定ウィンドウの情報タブ (`panels.js`) に載せた著作権表記とライセンス形態が `LICENSE` 冒頭と一致することを検証 (`LICENSE` だけ更新した取り残しを検出。Node.js 不要) |
| `architecture_doc_flow.rs` | `docs/dev/architecture.md` の構成図・基準ディレクトリ付きの表・バッククォート内のパスが実在することを検証 (移動・改名後の記述の取り残しを検出) |
| `oneshot_flow.rs` | ワンショット相当のパイプライン加工・常駐委譲時の監視エコー回避 |
| `oneshot_ipc_flow.rs` | 実名前付きパイプ IPC の往復 (加工成功 / 変更なし / 正規表現上書き / 複数ステッププリセット / 空パイプライン / 不正 JSON / ワーカー停止 / 過大要求 / 連続委譲 / 無応答接続の並行 / 同時処理上限からの回復 / 読み取り期限による無応答・途中送信接続の切断)。パイプ ACL の同一ユーザー接続は `ipc/server.rs` ユニットでも検証 |

OCR / 画面キャプチャの直接テストは統合テストではなく、次のモジュール内ユニットテストで行う。

| モジュール | 検証内容 |
| :--- | :--- |
| `platform/screen_capture/capture.rs` | 空矩形拒否、BGRA→RGBA 変換、仮想画面境界、左上小領域の実キャプチャ |
| `platform/ocr/recognize.rs` | 空画像拒否、RGBA→BGRA 変換、`SoftwareBitmap` 生成、単色画像でのパイプライン完走 (エンジン未導入時は初期化失敗を許容) |
| `platform/ocr/prepare.rs` / `normalize.rs` | 小画像拡大・最近傍補間・日本語スペース除去 |
| `platform/ocr_overlay/overlay_window.rs` | 生成・表示・非表示、マウスメッセージ注入による範囲確定、Escape キャンセル、枠色ピクセル |
| `platform/ocr_overlay/layered.rs` | 選択範囲の透明化、暗転復元、透明穴+枠色のプレビュー画素 |
| `platform/ocr_overlay/geometry.rs` | 矩形正規化、仮想デスクトップ座標変換、`LPARAM` 往復 |
| `tray/ocr/capture.rs` | 擬似表示トグル、`OcrJobGate` 世代破棄・スロット待ち後破棄 |

core クレートの公開加工 API は `crates/clip-refiner-core/tests/` で検証する。設定仕様 (スキーマ番号・既定値・上限) と `docs/user/configuration.md` / README / 生成 TOML の同期は `crates/clip-refiner-core/src/config/spec.rs` のユニットテストが担う。設定項目・上限・既定ホットキーを変えたら、実装・生成コメント・Markdown を同じ変更に含める ([ドキュメント同期](contributing.md#ドキュメント同期))。

| ファイル | 検証内容 |
| :--- | :--- |
| `refine_modes.rs` | 全 `RefineMode` の代表入力・出力とモード追加時のカバレッジ |
| `pipeline_api.rs` | テキスト / クリップボードのパイプライン API |
| `property_transforms.rs` | `proptest` による任意入力で成り立つ性質 ([プロパティテスト](#プロパティテスト-proptest)) |

UI ロジックテストは Node.js 未インストール時にスキップする (`note:` を出力)。`check.ps1` は Node.js を検証し `CLIP_REFINER_REQUIRE_NODE=1` で必須化する。意図的にスキップする場合は `CLIP_REFINER_SKIP_NODE_TESTS=1` を設定する。

設定 / セレクター WebView の回帰検知は次の **4 層**で行う。

| 層 | 手段 | カバー範囲 |
| :--- | :--- | :--- |
| JS 純粋ロジック | `settings_*_logic` / `regex_preview_logic` / `selector_logic` (Node.js) | パッチマージ、再描画判定、件数クランプ、閉じる IPC (`apply`→`close`)、Escape / ショートカット差分更新、正規表現プレビュー (タイムアウト表示含む)、パネル HTML / クリック委譲、セレクター検索・選択・行組み立て |
| Rust IPC / 組み立て | `settings_flow` / `settings/window/` / `settings/ipc/` / `settings/geometry.rs` / `settings/apply/patch/selector_webview.rs` / `selector/{window,lazy,quick,clip,retention}.rs` / `app_control` / `favorites` / `paths` ユニット | `apply` パッチ適用と `config.toml` 往復、HTML/JS インライン展開、シェル DOM・タブ定義、`closeWindow` 公開、ウィンドウ幾何 JSON 往復、セレクターアセット埋め込み・保持方針 (`Keep` / `DisposeOnHide` / `DisposeAfterIdle`)、トレイ「設定」メニュー、お気に入り並び→`modes_to_json_list`、`open_config_file` 起動パスキャプチャ |
| Playwright E2E | `e2e/*.spec.ts` (`npm run test:e2e`) | 実 WebView2 の描画・入力・フォーカス・矢印/Enter・Escape 閉じる、設定の debounce 保存と外部 `config.toml` バナー、自動変換と連続記録の排他 (チェック・バナーの反映→TOML)、お気に入り並べ替え→TOML、登録クリップの空表示・検索・並べ替えボタン・テンプレートバッジ・画像サムネ/ホバー、設定ウィンドウの登録クリップ編集ダイアログ (検証・保存失敗の表示・Esc・一覧変更で閉じる)、クイックのお気に入り順と `Ctrl+Shift+↑/↓` 並べ替え→TOML・`Ctrl+Enter`・連続記録中の `Ctrl+Enter` による自動変換への切替→TOML・カテゴリ見出しの開閉と再表示時の復元 (`settings.spec.ts` / `quick_selector.spec.ts` / `clip_selector.spec.ts`)。`--e2e-window` + CDP (`CLIP_REFINER_E2E_CDP_PORT`) |
| OCR / ホットキー (Rust) | `platform/ocr_overlay/*` / `tray/ocr/capture.rs` / `tray/hotkey/tests.rs` / `ocr_flow.rs` | オーバーレイ生成・表示、マウスメッセージ注入による範囲確定/Escape キャンセル、選択プレビュー画素 (透明穴+枠色)、`OcrJobGate` 世代破棄、ホットキーによる OCR/セレクター相互排他、OCR 結果のクリップボード書き込み |

OCR オーバーレイは WebView ではない (Win32 レイヤードウィンドウ)。ライフサイクル・ドラッグ確定・枠描画は上記 Rust テストで回帰検知する。キャプチャ後の OCR は `tray/ocr/capture.rs` の `OcrJobGate` で同時 1 本に直列化し、新しい範囲選択で旧世代の結果適用を破棄する。

### プロパティテスト (`proptest`)

`refine_modes.rs` は各モード 1 つの代表入力しか見ないため、境界ケース (空行・単独の CR・大きな数値・記号だけのセル・区切り文字と camelCase の混在など) は `property_transforms.rs` で任意入力から検出する。変換モードを追加・変更したら、次のどの性質が成り立つかを考えて該当する節へ追加する。

| 性質 | 例 |
| :--- | :--- |
| 件数・内容の保存 | 並べ替えは行数と行の集合を変えない。Markdown 表→Excel は行とセルを保つ。重複行削除・空行削除は残すべき行を残す |
| 往復 | JSON→YAML→JSON (128 bit 整数・浮動小数点・任意のキー)、半角⇄全角、TSV⇄Markdown 表、URL・エスケープ |
| 冪等性 (`f(f(x)) == f(x)`) | トリム・行操作・JSON 整形・全角半角・パス区切り・カンマ除去・ケース変換 |
| パニックしない | 任意の文字列と、生成した JSON / Markdown 表 / TSV / CSV で全モードが結果を返す |

性質が成り立たない入力を意図して除く場合 (camelCase で境界が消える 1 文字の単語など) は、生成器のコメントに理由を書く。ケース数は `PROPTEST_CASES` で増やせる (既定 256)。失敗した入力は `tests/*.proptest-regressions` に保存され、次回以降も先に再実行される。

```powershell
cargo test -p clip-refiner-core --test property_transforms
$env:PROPTEST_CASES = '5000'; cargo test -p clip-refiner-core --test property_transforms
```

### ファジング (`cargo-fuzz`)

CSV・Markdown 表・JSON・YAML・HTML のパーサ系変換は `fuzz/` の cargo-fuzz (libFuzzer) ターゲットで検証する。nightly が必要なためルートのワークスペースには含めず、`check.ps1` にも組み込まない。変換やパーサ依存を変えたときや、リリース前などにローカルで不定期に実行する。

前提:

```powershell
rustup toolchain install nightly --profile minimal
cargo +nightly install cargo-fuzz --locked
```

Visual Studio の「C++ によるデスクトップ開発」(AddressSanitizer を含む) も必要。Windows の cargo-fuzz は ASan を有効にしてビルドし、実行時に `clang_rt.asan_dynamic-x86_64.dll` を読み込む。`fuzz.ps1` が `vswhere` で MSVC のフォルダを探して `PATH` へ追加する。

```powershell
# 全ターゲットを 60 秒ずつ実行
./scripts/windows/fuzz.ps1

# ターゲットと時間を指定
./scripts/windows/fuzz.ps1 -Target line_sort -Seconds 600

# 失敗した入力を再実行
./scripts/windows/fuzz.ps1 -Target line_sort -Reproduce fuzz/artifacts/line_sort/crash-0123456789abcdef
```

| ターゲット | 検証内容 |
| :--- | :--- |
| `all_modes` | 全モードがパニックしない (正規表現は既定と置換設定ありの 2 通り) |
| `line_sort` | 並べ替えがパニックせず、行数と行の集合を変えない (CSV レコード単位の経路を含む) |
| `markdown_table` | Markdown 表⇄Excel がパニックしない (互いの出力を入力にした経路を含む) |
| `json` | JSON 整形が値を変えない。JSON→YAML の結果を YAML→JSON で読み戻せる |
| `yaml` | YAML→JSON がパニックせず、出力が JSON として解釈できる |
| `html` | HTML⇄Markdown がパニックしない (深い入れ子によるスタックあふれを含む) |

生成した入力は `fuzz/corpus/<ターゲット>/` に蓄積し、次回の実行で再利用する。失敗した入力は `fuzz/artifacts/<ターゲット>/` に保存する (どちらも Git 管理外)。Windows ではパニック時の `abort` を libFuzzer が捕捉できないため、`fuzz/src/lib.rs` のパニックフックが実行中の入力を `crash-<ハッシュ>` として保存する。見つかった不具合は、修正と同じ変更で `property_transforms.rs` かモジュール内ユニットテストに回帰ケースとして追加する。

### ベンチマーク (`criterion`)

fuzz とは別の観点で、巨大入力での処理時間の悪化を検知する。`crates/clip-refiner-core/benches/transforms.rs` が、パーサ入力上限 (1 MiB) の手前までの入力で次のモードを計測する。

| 計測対象 | モード |
| :--- | :--- |
| JSON・YAML | JSON 整形 (順序保持の有無) / JSON→YAML / YAML→JSON |
| Markdown・HTML・表 | Markdown→HTML / Markdown 表→Excel / HTML→Markdown |
| 行・文字列 | 昇順ソート / URL エンコード / 半角→全角 |
| 正規表現 | 置換 (マッチタイムアウト付きの実行パス) |

入力サイズは 64 KiB と 960 KiB の 2 通り。サイズ比に対して処理時間が線形から外れていないか (入力 15 倍で 15 倍前後か) を見る。計測前に各入力が実際に加工されること (上限超過や変換失敗で素通りしていないこと) を検証するため、入力生成や上限を変えて素通りすると失敗する。この検証は `cargo test --all-targets` でも各ベンチを 1 回ずつ実行して行うため、`check.ps1` でベンチのコードが壊れていないことは確認できる。計測自体は時間がかかりマシン負荷でぶれるため、`check.ps1` には含めずローカルで不定期に実行する。変換やパーサ依存を変えたとき、リリース前などに、変更前の基準と比較する。

```powershell
# 変更前のコミットで基準を保存する
./scripts/windows/bench.ps1 -Save before

# 変更後に基準と比較する (悪化したベンチを一覧する。終了コードには反映しない)
./scripts/windows/bench.ps1 -Compare before

# ベンチ名の一部で絞る
./scripts/windows/bench.ps1 -Filter json_format
```

基準は `target/criterion/` に保存する (Git 管理外)。ぶれの範囲に収まる差は「有意な変化なし」と表示される。悪化を示した場合は再計測して再現を確かめる。HTML レポートは `target/criterion/report/index.html` に出力する。モードを追加・変更して性能が気になる場合は、`transforms.rs` の `bench_transforms` にケースを足す。

### WebView E2E (Playwright)

対話 GUI セッションと WebView2 Runtime が必要 (ヘッドレス非対応)。`check.ps1` / pre-push でデフォルト実行する。

```powershell
cargo build --all-targets --features test-helpers
npm run test:e2e
```

スキップ: `CLIP_REFINER_SKIP_E2E=1`

ハーネスは `ClipRefiner.exe --e2e-window settings|quick|clip` を一時設定ディレクトリ (`CLIP_REFINER_CONFIG_DIR`) で起動し、stdout の `E2E_READY` のあと Playwright が CDP 接続する。実行ファイルパスは `CLIP_REFINER_E2E_EXE` で上書きできる (未設定時は `target/debug/ClipRefiner.exe`)。

ハーネスは終了時に、一時設定ディレクトリと `%TEMP%` の `ClipRefiner-*-WebView2-e2e-<ポート>` (WebView2 のデータ) を削除する。強制終了で残った分は、次の実行の開始時 (`e2e/global-setup.ts`) に、作成から 10 分以上たったものだけ削除する。

### 手動確認が残る範囲

設定 / セレクター / OCR ドラッグ / 画像プレビュー / お気に入り並びは上記の自動テストでカバーする。次は OS / 実機依存のため自動化していない。

| 項目 | 理由 |
| :--- | :--- |
| トレイアイコンの実クリックでメニューが開くこと | OS シェルのトレイ UI。メニュー ID → イベント経路はユニットで検証 |
| 既定エディタの実ウィンドウ表示 | `open_config_file` は起動引数をキャプチャテスト済み。実エディタ UI は環境依存 |
| グローバルホットキーの実キー押下 | OS 登録は `platform_flow`、合成イベントは `hotkey/tests`。他アプリとの競合は実機のみ |
| トースト通知の見た目 | 文言組み立てはユニット済み。OS トースト描画は環境依存 |
| 連続記録のフォアグラウンド貼り付け (`SendInput`) | 注入ロジックはユニット済み。アクティブウィンドウへの実貼り付けは実機のみ |
| アプリ更新の `msiexec` / 差し替えヘルパー実起動 | `update_flow` はモック bridge で確認〜検証〜展開まで固定。外部プロセス起動と再起動後の実フォルダ差し替えは実機のみ |

### テスト用ヘルパー (`test_helpers`)

`clip_refiner::test_helpers` はデバッグビルド、`cargo test` 実行時、または `test-helpers` feature 有効時に利用可能 (feature `app` が必要)。リリースビルドで Clippy・テスト・ドキュメントテストを走らせる場合は `--features test-helpers` を付ける (`cargo test --release --all-targets --features test-helpers` も通る)。テスト専用の公開シンボル (`ipc::ClientError`、`apply_update_at_with_key` など) も同じ条件に限定し、通常のリリースビルドでは未使用警告を出さない。

| モジュール | 所在 | 内容 |
| :--- | :--- | :--- |
| `clip_refiner::test_helpers` | `clip-refiner/src/test_helpers/` | アプリ層の統合テスト用 (`ClipboardHarness`, `TrayTestContext`, `SettingsTestContext` / `with_persistence`, `update` モック bridge、oneshot IPC など) |
| `clip_refiner_core::test_helpers` | `clip-refiner-core/src/test_helpers/` | コア単体テスト用 (`InMemoryTextClipboard`, `with_temp_config_dir`) |

アプリ側の `test_helpers` は core のヘルパーを re-export する。設定ディレクトリを触るテストは `with_temp_config_dir` で一時ディレクトリへ切り替える。

#### `ClipboardHarness`

監視ループ (`handle_clipboard_update`) とワーカーコマンド (`handle_command`) を、OS クリップボードなしで横断検証するハーネス。`InMemoryTextClipboard` と `AppState` (テスト用初期化) を束ね、本番と同じ処理関数を直接呼び出す。

```rust
use clip_refiner::test_helpers::ClipboardHarness;
use clip_refiner_core::RefineMode;

let mut harness = ClipboardHarness::with_text("  hello  ")
    .with_mode(RefineMode::Trim);

assert!(harness.run_monitor_update(false));
assert_eq!(harness.clipboard_text(), "hello");

harness.undo();
assert_eq!(harness.clipboard_text(), "  hello  ");
```

**ビルダー (設定の事前セット)**

| メソッド | 用途 |
| :--- | :--- |
| `with_html` | クリップボードに置く HTML 形式 (`html-to-markdown` の入力) |
| `with_external_copy_after_read` | 本文の読み取り直後に別の内容がコピーされた状況 (加工中の割り込みコピー) |
| `with_mode` | 監視・手動加工のデフォルトモード |
| `with_pipeline` | 監視時の加工パイプライン |
| `with_history` | 履歴の有効・無効 |
| `with_continuous_record` | 連続記録の有効・無効 |
| `with_monitor_mode` | `MonitorMode::Polling` / `Event` |

**監視ループ経路**

| メソッド | 本番の対応 |
| :--- | :--- |
| `run_monitor_update(event_driven)` | 監視ループ 1 ティック (`event_driven` は OS イベント有無) |
| `run_configured_monitor_update()` | 設定の `monitor_mode` に応じた `event_driven` で実行 |
| `replace_clipboard` / `reset_clipboard` | 外部コピー・クリップボード差し替えの模倣 |

戻り値 `bool` は加工が実行されたか (`handle_clipboard_update` と同じ)。

**ワーカーコマンド経路** (`ClipboardCommand` 相当)

| メソッド | コマンド |
| :--- | :--- |
| `process_mode` | `ProcessMode` |
| `process_oneshot_pipeline` | `ProcessOneshotPipeline` (ワンショットの常駐委譲) |
| `set_text` | `SetText` (履歴復元) |
| `set_ocr_text` | `SetOcrText` |
| `undo` | `Undo` |
| `register_clip_from_clipboard` | `RegisterClipFromClipboard` |
| `copy_registered_clip` | `CopyRegisteredClip` |
| `discharge_continuous_record_on_paste` | `PasteContinuousRecordFront` (`Alt+Shift+X` 経路と同じ。書き込み後に `RecordEvent::SendPaste` も送る) |
| `discharge_continuous_record_at` | `DischargeContinuousRecordAt` |

**検証用アクセサ**

`clipboard_text`, `history_len`, `history_entries`, `continuous_record_len`, `continuous_record_entries`, `registered_clip_count`, `registered_clip_text`, `take_undo_source`, `matches_last_written`, `with_config_mut` など。

新しい統合テストを追加するときは、既存の `*_flow.rs` に近い経路かどうかでファイルを選び、監視とワーカーの両方にまたがるケースは `ClipboardHarness` で往復検証する。トレイメニュー / メニューイベントは `TrayTestContext`、設定ウィンドウの IPC とパッチ適用は `SettingsTestContext` を使う。

#### `TrayTestContext`

`AppState`・`TrayMenu`・スタブ `ClipboardWorkerHandle` を束ねたトレイ単体テスト用ヘルパー。メニューイベントやパッチ適用など、メニューとワーカーが揃う経路向け。メニュー構築前に設定を変えたいときは `with_config`、履歴投入など状態を先に用意するときは `from_state` を使う。メニューだけ必要なら `test_tray_menu`、ワーカーだけなら `stub_worker` も利用できる。

```rust
use clip_refiner::test_helpers::TrayTestContext;

let ctx = TrayTestContext::with_config(|c| c.history_enabled = true);
assert!(ctx.menu().history.enabled_item.is_checked());
```

#### `SettingsTestContext`

設定ウィンドウの `apply` IPC・適用待ちキューのマージ・`config.toml` へのパッチ反映を、WebView なしで検証するヘルパー。`TrayMenu` とスタブ `ClipboardWorkerHandle` を束ね、本番と同じ `apply_settings_patch_without_ui` を呼び出す。ディスク往復は `with_persistence` を使う。

```rust
use clip_refiner::test_helpers::SettingsTestContext;

let ctx = SettingsTestContext::new();

assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"historyEnabled":true}}"#));
assert!(ctx.state().with_config(|c| c.history_enabled));
```

| メソッド | 用途 |
| :--- | :--- |
| `enqueue_apply_ipc` | JSON IPC の `apply` を適用待ちキューへ積む |
| `enqueue_patch_json` | 部分更新 JSON をキューへ積む (マージ検証用) |
| `take_pending_patch_json` | マージ済みパッチ JSON を取り出す |
| `apply_pending_patch` | キューから取り出して反映 |
| `apply_ipc` | JSON IPC を解釈して即時反映 |
| `with_persistence` | 一時 `config.toml` 上で永続化付きコンテキストを実行 |
| `state` / `menu` | 検証用アクセサ |

#### ワンショット IPC (`oneshot_ipc_flow`)

本番パイプ名と衝突しないテスト用パイプで `OneshotIpcServer` を起動し、クライアント往復を検証する。クリップボード I/O はスタブワーカー (`spawn_oneshot_stub_responder`) で差し替え、トランスポートとプロトコル境界に焦点を当てる。通信失敗は `ClientError` (`thiserror`) として返す (テストヘルパー経由で利用)。

| ヘルパー | 用途 |
| :--- | :--- |
| `start_oneshot_ipc_for_test` | 一意なパイプ名で待受開始 (`WaitNamedPipeW` で準備完了を待つ) |
| `start_oneshot_ipc_with_read_timeout_for_test` | 要求の読み取り期限を短くして待受開始 (無応答接続の切断検証用) |
| `process_oneshot_via_pipe` | 指定パイプへ正式な加工要求を送る |
| `send_raw_oneshot_line` | 不正 JSON / 過大要求など生 1 行を送る |
| `spawn_oneshot_stub_responder` | `ProcessOneshotPipeline` に固定結果を返す |
| `wait_for_oneshot_pipe` | 連続委譲前など、次インスタンスの待受開始を待つ |

## 品質チェック

`scripts/windows/check.ps1` で品質チェック一式を実行する。コード整形だけなら `scripts/windows/format.ps1` で rustfmt と Prettier を適用する。

```powershell
./scripts/windows/check.ps1
./scripts/windows/format.ps1
# WebView E2E のみスキップする場合
$env:CLIP_REFINER_SKIP_E2E = '1'; ./scripts/windows/check.ps1
# テストをカバレッジ計測付きで実行する場合 (cargo-llvm-cov)
./scripts/windows/check.ps1 -Coverage
# または
$env:CLIP_REFINER_COVERAGE = '1'; ./scripts/windows/check.ps1
```

`check.ps1` は次の順に実行し、最初の失敗で止まる。

| 順 | 内容 | 相当するコマンド |
| :--- | :--- | :--- |
| 1 | Rust の整形検証 | `cargo fmt --check` |
| 2 | Clippy (アプリ全体 + core の feature なし) | `cargo clippy ... -- -D warnings` |
| 3 | サプライチェーン監査 | `cargo deny check` |
| 4 | Node.js の確認 (`CLIP_REFINER_REQUIRE_NODE=1` を設定) | — |
| 5 | UI バンドルの最新性・ESLint・Prettier | `npm run build:ui:check` / `npm run lint` / `npm run format:check` |
| 6 | テスト (doctest 含む。`-Coverage` 時は `cargo llvm-cov`) | `cargo test --all-targets --features test-helpers` ほか |
| 7 | WebView E2E (`CLIP_REFINER_SKIP_E2E=1` でスキップ) | `cargo build --all-targets --features test-helpers` → `npm run test:e2e` |

E2E 用のビルドは対象と feature を `cargo test` と揃え、互いのビルド結果を無効にし合わないようにする (`-p clip-refiner` だけでは dev-dependencies の feature が統合されず、毎回再コンパイルになる。デバッグビルドでは `test-helpers` の有無で挙動は変わらない)。テスト・E2E 用ビルド・カバレッジの cargo は、リンカーがライブラリを開けない一過性の失敗 (`LNK1104`。並列リンク中にウイルス対策ソフトのスキャンと競合して起きると推定) のときだけ 1 回再実行する。判定のため出力を行単位で中継するので、cargo の進捗バーと色は出ない。

個別に実行する場合:

```powershell
cargo fmt --check
cargo clippy --all-targets --features test-helpers -- -D warnings
cargo clippy -p clip-refiner-core --no-default-features --all-targets -- -D warnings
cargo deny check
npm ci
npm run build:ui:check
npm run lint
npm run format:check
cargo test --all-targets --features test-helpers
cargo test -p clip-refiner-core --no-default-features --all-targets
cargo test --doc --features test-helpers
# カバレッジ (任意。coverage.ps1 と同条件)
cargo llvm-cov --all-targets --features test-helpers --ignore-filename-regex '(^|[/\\])(tests|test_helpers)([/\\]|$)|[/\\][^/\\]*_tests\.rs$|[/\\]tests\.rs$'
```

### Rust カバレッジ (`cargo-llvm-cov`)

ローカルで本番コードのテスト厚みを確認する。CI には組み込まず、必要なときだけ実行する。

前提:

```powershell
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

```powershell
# 端末にファイル別の行 / 領域カバレッジ要約を出す
./scripts/windows/coverage.ps1

# HTML レポートを target/llvm-cov/html/ に出力
./scripts/windows/coverage.ps1 -Html

# HTML を生成して既定ブラウザで開く
./scripts/windows/coverage.ps1 -Open
```

挙動の要点:

- `cargo test --all-targets --features test-helpers` 相当を LLVM 計測付きで実行する
- 集計から `tests/`・`test_helpers/`・`*_tests.rs` / `tests.rs` を除外し、本番ソースの厚みを見やすくする
- doctests は `cargo-llvm-cov` では不安定なためカバレッジ対象外とし、スクリプト末尾で通常の `cargo test --doc` を別実行する
- JS ロジックテストは `CLIP_REFINER_REQUIRE_NODE=1` で必須化する (`test.ps1` と同様)
- HTML 出力先は `target/llvm-cov/html/index.html` (`CARGO_TARGET_DIR` 設定時はその配下。Git 管理外)

薄いモジュールを探すときは、要約の低いファイルから HTML の行ハイライトへ進むとよい。しきい値での失敗 (`--fail-under-lines` 等) は設けない。

### Git フック

コミット・プッシュ時に品質チェックを自動実行する Git フックをリポジトリ同梱している。クローン後に一度だけ有効化する:

```powershell
./scripts/windows/install-hooks.ps1
```

| フック | 実行内容 | 所要時間の目安 |
| :--- | :--- | :--- |
| `pre-commit` | `cargo fmt --check` (`check-pre-commit.ps1`) | 数秒 |
| `pre-push` | `check.ps1` 一式 (fmt / clippy / deny / eslint / test / e2e) | 数分〜十数分 |

無効化:

```powershell
./scripts/windows/uninstall-hooks.ps1
```

一時的なスキップ:

| 方法 | 用途 |
| :--- | :--- |
| `git commit --no-verify` | pre-commit のみスキップ |
| `git push --no-verify` | pre-push のみスキップ |
| `CLIP_REFINER_SKIP_HOOK=1` | 両フックをスキップ |

`core.hooksPath` はローカル Git 設定 (`git config core.hooksPath`) に保存される。フック本体は `scripts/git/hooks/` に置き、バージョン管理する。

手動でフックを試す場合:

```powershell
git hook run pre-commit
git hook run pre-push
```

### JavaScript リンター (ESLint / Prettier)

UI / テスト用 JavaScript (`crates/clip-refiner/src/ui/`, `crates/clip-refiner/tests/`) は ESLint と Prettier で検証する。`check.ps1` / `format.ps1` は `scripts/common/npm.ps1` 経由で実行する。

| ツール | 役割 |
| :--- | :--- |
| [ESLint](https://eslint.org/) | 推奨ルール + `eslint-plugin-no-unsanitized` (`innerHTML` 等の XSS リスク検出)。設定 / セレクター UI と Node ロジックテストは ESM (`import`/`export`)。ルート `package.json` は `"type": "module"` |
| [Prettier](https://prettier.io/) | JavaScript の整形 (タブ幅 4、シングルクォート) |
| [esbuild](https://esbuild.github.io/) | 設定 UI とセレクターの ESM を IIFE バンドル (`*.bundle.js`) にまとめる |

初回または `package.json` 更新後:

```powershell
npm ci
```

個別実行:

```powershell
npm run build:ui           # 設定 / セレクター UI バンドル再生成
npm run build:ui:check     # 設定 / セレクター UI バンドルが最新か検証 (書き込みなし)
npm run lint               # ESLint
npm run lint:fix           # ESLint (自動修正)
npm run format:check       # Prettier 整形チェック
npm run format             # Prettier 整形適用
```

設定ファイル: [`package.json`](../../package.json), [`eslint.config.js`](../../eslint.config.js), [`.prettierrc.json`](../../.prettierrc.json)

設定画面とセレクターの JavaScript は ESM (`import`/`export`) に分割し、`npm run build:ui` (`scripts/common/build-ui.mjs` / esbuild) で IIFE バンドル (`settings.bundle.js` / `quick_selector.bundle.js` / `clip_selector.bundle.js`) にまとめる。Rust は生成物を `include_str!` で埋め込むため、バンドルはリポジトリにコミットし、`cargo build` 単体では Node.js を要求しない。ソースだけ更新してバンドルを忘れる事故は 2 段で防ぐ。

1. バンドル先頭に入力ファイル一覧 (`build-ui.mjs` 自身を含む) と SHA-256 を刻み、`cargo test` の `ui_bundle_flow.rs` が Node.js なしで再計算して照合する
2. `check.ps1` の `npm run build:ui:check` が作業ツリーを書き換えずに再生成し、コミット済みバンドルとバイト比較する (手編集や esbuild の版差も検出する)

UI のファイル構成と `innerHTML` の扱いは [アーキテクチャ](architecture.md#webview-ui-srcui) を参照。

### サプライチェーン監査 (`cargo-deny`)

[`deny.toml`](../../deny.toml) で依存関係の脆弱性・ライセンス・取得元を検証する。`check.ps1` は `cargo deny check` を含む。

| チェック | 内容 |
| :--- | :--- |
| **advisories** | RustSec 公表の脆弱性・yanked crate。`unmaintained` はワークスペース直接依存のみ失敗 |
| **licenses** | 商用利用可能なパーミッシブライセンス (MIT / Apache-2.0 / BSD 等) のみ許可。GPL / LGPL / MPL 等のコピーレフトは拒否 |
| **bans** | 同一 crate の多重バージョンなど |
| **sources** | crates.io 以外のレジストリ / git 依存の拒否 |

`graph.all-features` と Windows ターゲット (`x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc`) を対象に、feature `app` 有効時の依存グラフを検査する。
