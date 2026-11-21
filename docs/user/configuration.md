# 設定リファレンス

`config.toml` の場所・設定項目と既定値・処理の制限など、設定ファイルの仕様をまとめる。日常的な変更は [設定ウィンドウ](usage.md#設定ウィンドウ) から行える。操作方法は [使い方](usage.md)、変換モードは [変換モード](modes.md) を参照。

現行の設定スキーマは `version = 5` 。スキーマの変更と移行内容は [変更履歴](changelog.md) の該当バージョンにある **Settings** / **Breaking changes** 節に記載する。

**目次:** [設定ファイルの場所](#設定ファイルの場所) · [最小構成例](#最小構成例) · [処理の制限](#処理の制限) · [設定項目](#設定項目) · [ホットキー形式](#ホットキー形式) · [通知](#通知-notification_settings) · [一時停止の廃止](#一時停止の廃止-v5) · [加工パイプライン](#加工パイプライン-pipeline) · [監視方式](#監視方式-monitor_mode) · [セレクター WebView の保持](#セレクター-webview-の保持-selector_webview_retention) · [ログ](#ログ)

## 設定ファイルの場所

設定ファイル (`config.toml`) は設定変更のたびに自動保存され、以下の場所に置かれる。MSI のインストール先 (`%LOCALAPPDATA%\ClipRefiner`) とは異なる。保存は同じディレクトリの一時ファイルへ書き込んでから置き換えるため、書き込み途中の終了でファイルが壊れにくい。保存に失敗した場合はデスクトップ通知で案内する。

解決の優先順位:

1. 環境変数 `CLIP_REFINER_CONFIG_DIR` (非空のとき。設定ディレクトリを明示的に指定する)
2. [ポータブルモード](#ポータブルモード) (`<実行ファイルのフォルダ>\data`)
3. 既定の `%APPDATA%\ClipRefiner`

| 利用形態 | パス |
| :--- | :--- |
| **通常** (既定) | `%APPDATA%\ClipRefiner\config.toml` |
| **ポータブル** | `<実行ファイルのフォルダ>\data\config.toml` |

設定ディレクトリ内の主なファイル / フォルダ:

| パス | 内容 |
| :--- | :--- |
| `config.toml` | メイン設定 (変換モード・ホットキー・通知など) |
| `config.toml.bak` | 解析に失敗した `config.toml` の退避コピー (起動時の読み込み、または保存による上書きの前に作る) |
| `registered-clips.dat` | 登録クリップ本文 (ChaCha20-Poly1305 暗号化) |
| `registered-clips.dat.bak` | 読み込めなかった (一部のクリップを読み込めなかった場合を含む) `registered-clips.dat` の退避コピー |
| `registered-clips.key` | 登録クリップ用鍵 (Windows DPAPI で保護) |
| `registered-images/` | 登録クリップ画像 (暗号化) |
| `settings-window.json` | 設定ウィンドウのサイズ・位置 |
| `logs/` | 日次ログ (`clip-refiner.log.YYYY-MM-DD`) |

### 読み込みの異常時の挙動

| 状況 | 挙動 |
| :--- | :--- |
| TOML の解析に失敗 | 壊れた元ファイルを `config.toml.bak` へ退避し、デフォルト設定で起動する。デスクトップ通知 (常駐) または標準エラー出力 (ワンショット) で案内する。`.bak` は正常だった前世代のバックアップではないため、そのまま戻さず構文を修正する |
| 常駐中に `config.toml` が外部で編集され、解析に失敗 | 現在の設定を維持し、デスクトップ通知で案内する。同じ内容では通知を繰り返さず、内容を直すと再び読み込む。直す前にアプリ側で設定が保存される場合は、上書きの前に壊れた内容を `config.toml.bak` へ退避する (退避コピーを作れない場合は保存を中止する) |
| `registered-clips.dat` を読み込めない (鍵の欠落・ファイルのロック・サイズ超過など) | 登録クリップを空の状態で起動し、元ファイルを `registered-clips.dat.bak` へ退避する。設定を保存しても `registered-clips.dat` は削除せず、登録クリップを追加して保存したときだけ上書きする (退避コピーを作れない場合は保存を中止する)。原因を取り除いて再起動すると読み込める |
| `registered-clips.dat` の一部のクリップだけ読み込めない (保存時より本文の上限が下がった旧形式のクリップなど) | 読み込めたクリップだけで起動し、元ファイルを `registered-clips.dat.bak` へ退避する。扱いは上の行と同じで、登録クリップを保存したときに、読み込めなかったクリップを除いた内容で上書きする |
| 移行手順が無いスキーマ `version` (現行より古く、対応する移行が未実装) | デフォルト設定へ置き換えず、既存項目を維持して警告する。ディスク上の `version` は自動では書き換えない (設定ウィンドウなどから保存したときだけ現行番号になる) |
| 現行より新しい `version` | 既存項目を維持し、番号だけ現行へ揃えて保存する |
| 認識できないキー (タイプミス・廃止名・未サポート項目) | 読み込みは継続し、既知の項目だけ適用する。起動時・再読み込み時にログへ記録し、通知 (常駐) または標準エラー出力 (ワンショット) でキー名を案内する。旧キー (`notify_mode` / `hotkeys.paste_queue` など) や別名は警告しない |

TOML 形式のため `#` でコメントを書ける。初回保存時は各項目の説明コメントが付き、以降の保存ではユーザーが追記したコメントを維持したまま値だけ更新する。設定ディレクトリとログファイルは、可能な場合に現在ユーザー専用 DACL で保護する (FAT / exFAT など ACL 非対応ボリュームでは保護をスキップし、警告だけ記録する)。

### ポータブルモード

実行ファイルと同じフォルダに、名前が `portable` のファイル (中身は空でよい) を置くと、設定・ログ・登録クリップを実行ファイル隣の `data\` へ保存する。フォルダごと USB メモリへコピーして持ち出せる。

```text
ClipRefiner.exe
portable          ← このファイルがある
data\
  config.toml
  logs\
  registered-clips.dat
  ...
```

有効化手順:

1. `ClipRefiner.exe` と同じフォルダに空の `portable` ファイルを作成する
2. アプリを起動する (初回は `data\` が自動作成される)
3. 必要なら `%APPDATA%\ClipRefiner` から `config.toml` や登録クリップ関連ファイルを `data\` へコピーする

注意:

- `CLIP_REFINER_CONFIG_DIR` が設定されているときはポータブルマーカーより優先される
- MSI インストール先へ `portable` を置くと、そのインストールの設定も `data\` 側へ切り替わる
- 登録クリップの鍵は Windows DPAPI (現在ユーザー) で保護されるため、別ユーザーや別 PC へフォルダを移すと復号できない場合がある
  - 復号できない間も `config.toml` の保存はできる。登録クリップの保存だけが省略され、既存の `registered-clips.dat` は変更されない
- ログイン時の自動起動はレジストリに実行ファイルの絶対パスを書くため、USB を抜き差しする用途には向かない
- 通知の表示名とアイコンを出すため、ポータブルでも起動のたびに現在のユーザーのレジストリ `HKCU\Software\Classes\AppUserModelId\com.y_hirata.clip-refiner` を更新し、アイコンを `data\notification-icon.ico` へ書き出す
  - 自動起動の設定とは無関係に書き込む。フォルダを削除してもキーは残るため、不要なら手動で削除する ([トラブルシューティング](troubleshooting.md#アンインストール))
- アプリ内の [更新](usage.md#アプリの更新) でポータブル ZIP を適用しても `data\` は保持される
- 起動ログに `ポータブルモード: 設定ディレクトリ = ...` と出る

### 外部編集の自動再読み込み

常駐中は `config.toml` の外部編集を検知し、内容が変わったときだけ自動で再読み込みする。同一秒の上書きや時計ずれでも反映し、アプリ自身の保存や同一内容の再保存では再読み込みしない。変更の検知を開始できない場合は、約 2 秒ごとの確認に切り替える。

- **解析失敗:** 起動時と異なりデフォルト設定へは戻さず、現在の設定を維持したまま通知でエラーを表示する
- **未知キー / 移行手順が無い `version`:** 反映は継続し、再読み込み通知に内容を含める
- **設定ウィンドウ表示中:** 外部更新をバナーで知らせる

## 最小構成例

初回起動でコメント付きの完全な `config.toml` が生成されるため、通常はそれを設定ウィンドウまたはエディタで変更する。手動で新規作成する場合の最小構成は次のとおり。

```toml
version = 5
mode = "UrlDecode"
interval_ms = 1000
```

`version` は設定スキーマの番号であり、アプリのバージョン番号ではない。手動で増減せず、現行値を維持する。変換モードは PascalCase で指定する。たとえば URL デコード後に前後の空白を除去する場合:

```toml
version = 5
auto_refine_enabled = true
mode = "UrlDecode"
pipeline = ["UrlDecode", "Trim"]
interval_ms = 1000
monitor_mode = "Event"

[notification_settings]
enabled = true
notify_result = false
```

利用できるキーと既定値は [設定項目](#設定項目) を参照。

## 処理の制限

| 対象 | 上限 |
| :--- | :--- |
| クリップボード本文 | 2 MiB |
| JSON / YAML / Markdown パーサー入力 | 1 MiB |
| HTML→Markdown の要素の入れ子 | 256 階層 |
| 正規表現パターン | 8 KiB (ステップ単位) |
| 正規表現プリセット | 20 件 |
| 正規表現ステップ / プリセット | 10 件 |
| 正規表現マッチ実行時間 | 2 秒 |
| 正規表現マッチ同時実行 | 2 本 |
| 履歴 / 連続記録モードの保持件数 | 1〜100 件 (既定 10) |
| 登録クリップ | 100 件 (ラベル 64 文字、テキスト 2 MiB、画像 PNG 16 MiB / 8192 px) |
| 登録クリップ暗号化ファイル | 256 MiB |
| Base64 と相互変換する画像 | 辺長 8192 px、16777216 ピクセル (4096×4096) まで |
| 貼り付け用に書き出す画像ファイル | 1 時間保持 |
| お気に入り変換モード | 20 件 |
| 加工パイプライン | 10 段 |
| ポーリング間隔 (`interval_ms`) | 100〜60000 ms |

- **上限超過時:** 入力は加工せず元のテキストを維持する。登録クリップの追加は拒否する
- **案内:** 監視・手動加工・ワンショットでは、本文上限超過・パーサー上限超過・HTML の入れ子上限超過・正規表現タイムアウトをデスクトップ通知 (タイトル「加工をスキップ」) または標準エラー出力で案内する。連続記録モードで本文上限を超えた場合も通知する
- **正規表現タイムアウト:** 加工を中断してクリップボードの元テキストを維持する (設定 UI のプレビューではタイムアウトとして表示)
- **JSON / YAML の変換失敗:** 不正な JSON / YAML、重複キー、YAML の `.inf` / `.nan` などは変換せず元のテキストを維持する。手動加工とワンショットでは「変更なし」ではなく理由を通知または標準エラー出力で案内する。自動変換 (監視) では通知しない

機密情報の表示マスキングは [表示マスキングの範囲](security.md#表示マスキングの範囲) を参照。

## 設定項目

`mode` などの変換モード名は、CLI の kebab-case (`url-decode` など) ではなく PascalCase (`UrlDecode` など) で指定する。CLI 名との対応は [変換モード一覧](modes.md#変換モード一覧) を参照。

`[hotkeys]` と `[notification_settings]` のキーは、camelCase で書いたもの (`quickSelector`・`notifyProcess` など) も同じキーとして読み込む。保存時は下表のキー名に置き換える。

### 基本

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `version` | number | `5` | 設定スキーマのバージョン |
| `auto_refine_enabled` | bool | `false` | コピー時に `mode` / `pipeline` で自動変換するかどうか。新規設定は無効で、v3 以前の設定から移行したときは有効になる。`continuous_record_enabled` とは同時に有効にならない。トレイ「変換モード → 自動変換を有効にする」・設定ウィンドウ「変換」タブ・`hotkeys.auto_refine` でも切り替えられる |
| `mode` | string | `"UrlDecode"` | 使用する変換モード (`pipeline` が空の場合に監視で適用) |
| `pipeline` | array | (空) | 監視時に順に適用する変換モードの連鎖 (PascalCase の配列、最大 10 段。空の場合は `mode` のみ。例: `["UrlDecode", "Trim"]`) |
| `favorite_modes` | array | (空) | お気に入り変換モード (PascalCase の配列、最大 20 件。例: `["UrlDecode", "Trim"]`) |

### 監視

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `interval_ms` | number | `1000` | クリップボードのポーリング間隔 (ミリ秒、100〜60000)。設定ウィンドウ「監視」タブで数値入力する |
| `monitor_mode` | string | `"Event"` | 監視方式。`"Polling"` または `"Event"` |

### セレクター WebView

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `selector_webview_retention` | string | `"Keep"` | セレクター WebView の保持方針。`"Keep"` (非表示後も保持) / `"DisposeOnHide"` (非表示時に破棄) / `"DisposeAfterIdle"` (アイドル後に破棄) |
| `selector_webview_idle_minutes` | number | `5` | `DisposeAfterIdle` 時に非表示から破棄するまでの分数 (1〜1440)。設定ウィンドウ「一般」タブの詳細設定でも変更可 |

### 履歴・連続記録モード

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `history_enabled` | bool | `false` | 加工履歴の有効・無効 |
| `history_limit` | number | `10` | 履歴の最大保持件数 (1〜100) |
| `continuous_record_enabled` | bool | `false` | 連続記録モード (監視時は加工せず元テキストを記録)。`auto_refine_enabled` とは同時に有効にならない |
| `continuous_record_limit` | number | `10` | 連続記録モードの最大保持件数 (1〜100) |

### 通知

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `notification_settings.enabled` | bool | `false` | デスクトップ通知の有効・無効 (エラー通知は常に表示) |
| `notification_settings.notify_process` | bool | `true` | 加工完了時の通知 |
| `notification_settings.notify_auto_refine` | bool | `true` | 自動変換の切替時の通知 |
| `notification_settings.notify_history_restore` | bool | `true` | 履歴復元時の通知 |
| `notification_settings.notify_ocr` | bool | `true` | OCR 完了時の通知 |
| `notification_settings.notify_registered_clip` | bool | `true` | 登録クリップ操作時の通知 |
| `notification_settings.notify_favorites` | bool | `true` | お気に入り操作時の通知 |
| `notification_settings.notify_continuous_record` | bool | `true` | 連続記録モード操作時の通知 |
| `notification_settings.notify_result` | bool | `false` | 通知にクリップボードの内容を表示するかどうか |

### ホットキー

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `hotkeys.quick_selector` | string | `"Alt+Shift+S"` | クイックセレクター表示 |
| `hotkeys.clip_selector` | string | `"Alt+Shift+T"` | 登録クリップセレクター表示 |
| `hotkeys.ocr` | string | `"Alt+Shift+G"` | 画面範囲選択 OCR の開始 |
| `hotkeys.notification` | string | `"Alt+Shift+N"` | デスクトップ通知のマスター ON/OFF (エラー通知は常に表示) |
| `hotkeys.auto_refine` | string | `"Alt+Shift+P"` | 自動変換の有効・無効切替 |
| `hotkeys.continuous_record` | string | `"Alt+Shift+C"` | 連続記録モードの有効・無効切替 |
| `hotkeys.continuous_record_undo` | string | `"Alt+Shift+Backspace"` | 連続記録モードの直近 1 件取り消し |
| `hotkeys.continuous_record_paste` | string | `"Alt+Shift+X"` | 連続記録モードのキュー先頭をクリップボードへ載せて貼り付け |
| `hotkeys.undo` | string | `"Alt+Shift+Z"` | 直近の加工を取り消し |
| `hotkeys.quit` | string | `"Alt+Shift+Q"` | アプリケーション終了 |
| `hotkeys.favorite_mode_slots` | array | (空) | お気に入り変換モード用ホットキー (登録順インデックスに対応。未指定スロットは Alt+Shift+1〜Alt+Shift+9 / Alt+Shift+F1〜Alt+Shift+F11。空文字で無効) |

### 正規表現

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `regex.active_preset_id` | string | `"default"` | 有効な正規表現プリセットの ID |
| `regex.presets` | array | (デフォルト 1 件) | 名前付きプリセット (`id` / `name` / `steps`)。最大 20 件。各 `steps` は最大 10 件で上から順に適用 |
| `regex.presets[].id` | string | `"default"` | プリセット識別子 (設定内で一意) |
| `regex.presets[].name` | string | `"デフォルト"` | トレイ・設定 UI での表示名 |
| `regex.presets[].steps[].pattern` | string | `""` | 正規表現パターン (最大 8 KiB) |
| `regex.presets[].steps[].replacement` | string | `""` | 置換文字列 (`regex-replace` で使用。`$1` 形式のキャプチャ参照可) |
| `regex.presets[].steps[].case_insensitive` | bool | `false` | 大文字小文字を無視 (`(?i)` 相当) |
| `regex.presets[].steps[].multiline` | bool | `false` | 複数行モード (`(?m)` 相当) |

### 登録クリップ

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| (登録クリップ) | — | — | `config.toml` には含まれない。設定ディレクトリ内の `registered-clips.dat` (暗号化) と `registered-images/` に保存。最大 100 件 |

## ホットキー形式

`Alt+Shift+S` のように、`+` 区切りで修飾キーとキーを指定する。

- **修飾キー**: `Alt`, `Shift`, `Ctrl` (`Control` 可), `Meta` (`Super` / `Win` 可)
- **キー**:
  - 英数字: `A`〜`Z`, `0`〜`9`
  - ファンクション: `F1`〜`F24`
  - 編集・移動: `Backspace`, `Space`, `Enter` (`Return` 可), `Tab`, `Escape` (`Esc` 可), `Insert` (`Ins` 可), `Delete` (`Del` 可), `Home`, `End`, `PageUp` (`PgUp` 可), `PageDown` (`PgDn` 可), `ArrowUp` / `ArrowDown` / `ArrowLeft` / `ArrowRight` (`Up` / `Down` / `Left` / `Right` 可)
  - その他: `PrintScreen`, `Pause`, `ScrollLock`
  - テンキー: `Numpad0`〜`Numpad9`, `NumpadAdd`, `NumpadSubtract`, `NumpadMultiply`, `NumpadDivide`, `NumpadDecimal`
  - 記号: `Minus` (`-`), `Comma` (`,`), `Period` (`.`), `Slash` (`/`)

キー名の大文字・小文字は区別しない。記号キーは US 配列と JIS 配列で同じ位置・同じ仮想キーになるものだけに対応する (`;` `[` `@` などは配列で仮想キーが変わるため対象外)。OS や他のアプリが使用中の組み合わせ (`Win+L` など) は登録に失敗する。

不正な値や重複は読み込み時・設定 UI 保存時にデフォルトへ置き換える (お気に入りスロットの空文字は意図的な無効化として維持)。設定ウィンドウの「ショートカット」タブから保存した場合は、補正した割り当て名を適用フィードバックで表示する。

**既定値の変更と既存設定:**

| 項目 | 現行の既定 | 備考 |
| :--- | :--- | :--- |
| `hotkeys.ocr` | `Alt+Shift+G` | 旧既定は `Alt+Shift+O`。`config.toml` に保存済みの値がある場合はそちらを維持する (キー欠落時のみ新既定を適用) |
| `hotkeys.continuous_record_paste` | `Alt+Shift+X` | 旧キー名 `paste_queue` は別名として読み込む。保存時は `continuous_record_paste` のみ出力する。連続記録モードの貼り付けは `Ctrl+V` 捕捉ではなくこのホットキー |
| `hotkeys.auto_refine` | `Alt+Shift+P` | v4 以前の `hotkeys.pause` (一時停止・再開) を引き継ぐ。旧キー名は別名として読み込み、保存時は `auto_refine` のみ出力する |

## 通知 (`notification_settings`)

デスクトップ通知は **マスター** (`notification_settings.enabled`) と **種類別トグル** で制御する。起動失敗・設定読み込み失敗・クリップボード初期化失敗などの **エラー通知はマスター OFF でも常に表示** する。

| 操作 | 動作 |
| :--- | :--- |
| 設定ウィンドウ「通知」タブ | マスターと個別項目を GUI で編集 (マスター OFF 時も個別設定は変更可能) |
| `Alt+Shift+N` | マスター ON/OFF のみ切替 (`hotkeys.notification`) |

種類別トグルと対象操作:

| キー | 既定 | 対象 |
| :--- | :--- | :--- |
| `notify_process` | `true` | 加工完了 (監視による自動変換と、クイックセレクター・お気に入りホットキーによる手動変換)・加工の取り消し・自動変換の切り替えによるパイプライン解除 |
| `notify_auto_refine` | `true` | 自動変換の有効・無効の切替 (連続記録モードとの排他は下記の注記を参照) |
| `notify_history_restore` | `true` | 履歴からの復元 |
| `notify_ocr` | `true` | 画面 OCR 完了 |
| `notify_registered_clip` | `true` | 登録クリップのコピー・登録 |
| `notify_favorites` | `true` | お気に入りの登録・解除 (登録上限の案内を含む) |
| `notify_continuous_record` | `true` | 連続記録モードの有効化・取り出しなど |
| `notify_result` | `false` | 上記通知本文にクリップボード内容を含めるか (マスターとは独立) |

自動変換と連続記録モードの一方を有効にしてもう一方が無効になったときは、両方の切替をまとめた通知を 1 件だけ表示する。この通知は `notify_auto_refine` と `notify_continuous_record` のどちらかが `true` なら表示する。

`notify_result` が `true` のときも、機密らしい内容は `[機密情報のため非表示]` にマスクする (通知本文の表示のみ。クリップボード本体は対象外)。加工パイプライン適用時の加工完了通知は、連鎖したモード名を連結して表示する ([加工パイプライン](#加工パイプライン-pipeline) 参照)。

**設定例:**

```toml
[notification_settings]
enabled = true
notify_process = true
notify_auto_refine = false
notify_history_restore = true
notify_ocr = true
notify_registered_clip = true
notify_favorites = true
notify_continuous_record = true
notify_result = false
```

**旧設定からの移行:** v0.12.1 以前の `notify_mode` / `notify_actions` は読み込み時に新項目へ自動移行し、次回保存時に TOML から除去される。変更経緯は [変更履歴](changelog.md) の v0.12.2 を参照。

| 旧キー | 移行先 |
| :--- | :--- |
| `notify_mode = false` かつ `notify_result` 未指定 | `notify_process = false` |
| `notify_actions = false` | `notify_history_restore` / `notify_ocr` / `notify_registered_clip` / `notify_favorites` / `notify_continuous_record` を `false` |

v5 で一時停止を廃止したため、`notify_pause` は `notify_auto_refine` へ引き継ぐ (旧キー名も別名として読み込む)。移行の詳細は [一時停止の廃止](#一時停止の廃止-v5) を参照。

## 一時停止の廃止 (v5)

設定スキーマ v5 で、監視の一時停止 (`is_paused`) を廃止し、コピー時の自動変換 (`auto_refine_enabled`) に一本化した。トレイの「一時停止」項目は無くなり、`Alt+Shift+P` は自動変換の有効・無効切替になる。

| v4 以前 | v5 |
| :--- | :--- |
| `is_paused = true` | `auto_refine_enabled = false` へ移行し、`is_paused` は保存時に除去する |
| `hotkeys.pause` | `hotkeys.auto_refine` へ引き継ぐ (旧キー名も別名として読み込む) |
| `notify_pause` | `notify_auto_refine` へ引き継ぐ (旧キー名も別名として読み込む) |

旧キー名と新キー名が同じテーブルに両方ある場合は、新キー名の値を使い、旧キー名は無視する。

**自動変換と連続記録モードの排他:** `auto_refine_enabled` と `continuous_record_enabled` は同時に有効にならない。一方を有効にすると、もう一方は自動で無効になり、通知する (後から有効にした方が優先)。`config.toml` で両方 `true` の場合は、連続記録モードを優先して自動変換を無効にする。v4 以前から移行する設定も同様に扱う。

## 加工パイプライン (`pipeline`)

`pipeline` に複数の変換モードを指定すると、クリップボード監視時に左から順に連鎖適用する。空の場合は `mode` だけを適用する。どちらも `auto_refine_enabled = true` のときだけ監視で使われる。

- `pipeline` が非空のときに `mode` だけを変更しても、監視で適用される連鎖は変わらない (`mode` は次に `pipeline` をクリアしたときに使われる)
- トレイメニューまたはクイックセレクターの `Ctrl+Enter` でモードを選択すると `pipeline` はクリアされ、単一モードへ切り替わる。同時に `auto_refine_enabled` も有効になる
- [設定ウィンドウ](usage.md#設定ウィンドウ) の「変換」タブでもパイプラインとデフォルトモードを編集できる

```toml
mode = "UrlDecode"
pipeline = ["UrlDecode", "Trim", "JsonFormat"]
```

- **最大段数**: 10 段まで。超える分は切り捨てる (`ExcelToImage` は残す)
- **画像出力**: `ExcelToImage` はパイプライン末尾へ自動移動
- **HTML 形式**: `HtmlToMarkdown` は先頭のときだけクリップボードの HTML 形式を読み、`MarkdownToRichText` はテキスト加工の最終段のときだけ HTML 形式を書き込む ([リッチテキストと HTML 形式](modes.md#リッチテキストと-html-形式) 参照)
- **通知**: 連鎖適用時は `URLデコード → 全体をトリム → JSON整形` のようにモード名を連結して表示

ワンショット実行時は CLI の `--pipeline` で同様に連鎖を指定できる (`--mode` より優先。[ワンショットモード](usage.md#ワンショットモード) 参照)。例:

```powershell
.\ClipRefiner.exe --pipeline url-decode trim json-format
```

## 監視方式 (`monitor_mode`)

| 方式 | 説明 |
| :--- | :--- |
| `Event` | (既定) Windows のクリップボード更新通知で変更を受け取り、変更時のみ本文を読む。ポーリングより低遅延かつ低 CPU 負荷。通知を使えない場合だけ、短周期の監視に切り替える |
| `Polling` | 一定間隔 (`interval_ms`) でクリップボードの内容を読み取り、変更を検知する方式 |

設定ウィンドウ「監視」タブで、監視方式とポーリング間隔 (`interval_ms`、100〜60000 ms) を変更できる。`Event` のときポーリング間隔は使われず、入力欄は無効になる。

## セレクター WebView の保持 (`selector_webview_retention`)

クイックセレクター / 登録クリップセレクターの `WebView` を、非表示後どう扱うか。再表示の速さ (保持) とアイドル時のメモリ (破棄) のトレードオフ。設定ウィンドウと画面 OCR オーバーレイは従来どおり非表示時に破棄する (この設定の対象外)。

| 値 | 動作 |
| :--- | :--- |
| `Keep` (既定) | 非表示後も `WebView` を保持し、再表示を高速化する (アイドル時の RSS は増える) |
| `DisposeOnHide` | 非表示時に `WebView` を破棄する |
| `DisposeAfterIdle` | 非表示から `selector_webview_idle_minutes` 分 (1〜1440、既定 5) 経過後に破棄する |

設定ウィンドウ「一般」タブの詳細設定からも変更できる。`DisposeAfterIdle` のときだけアイドル分数入力が有効になる。

```toml
# 再表示を優先 (既定)
selector_webview_retention = "Keep"

# メモリを優先する場合の例
# selector_webview_retention = "DisposeAfterIdle"
# selector_webview_idle_minutes = 5
```

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存する。ファイル名の日付 (`clip-refiner.log.YYYY-MM-DD`) はローカルタイムゾーン基準。不具合報告時は該当する日付のログファイルを添付する。14 日より古いログファイルは、起動時と日付が変わったときに自動で削除する。

| 利用形態 | パス |
| :--- | :--- |
| **通常** (既定) | `%APPDATA%\ClipRefiner\logs\` |
| **ポータブル** | `<実行ファイルのフォルダ>\data\logs\` |

アプリが異常終了 (パニック) した場合も、発生スレッド・メッセージ・発生位置をログファイルへ記録する。
