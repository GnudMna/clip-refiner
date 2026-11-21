# ClipRefiner 設定リファレンス

ClipRefiner は **Windows 専用**です。ユーザ向けの概要・クイックスタートは [README.md](README.md) を参照してください。ビルドや開発手順は [DEVELOPMENT.md](DEVELOPMENT.md) を参照してください。

## 目次

- [設定ファイルの場所](#設定ファイルの場所)
- [処理の制限](#処理の制限)
- [設定項目](#設定項目)
- [加工モード一覧](#加工モード一覧)
- [加工モードの使用例](#加工モードの使用例)
- [ホットキー形式](#ホットキー形式)
- [通知 (`notification_settings`)](#通知-notification_settings)
- [UI 操作](#ui-操作)
- [設定ウィンドウ](#設定ウィンドウ)
- [画面 OCR](#画面-ocr)
- [加工パイプライン (`pipeline`)](#加工パイプライン-pipeline)
- [監視方式 (`monitor_mode`)](#監視方式-monitor_mode)
- [連続記録](#連続記録)
- [クリップボード履歴のセキュリティ](#クリップボード履歴のセキュリティ)
- [ログイン時の自動起動](#ログイン時の自動起動)
- [ログ](#ログ)

---

## 設定ファイルの場所

設定ファイル (`config.toml`) は設定変更のたびに自動保存され、以下の場所に配置されます。

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\config.toml` |

**設定ディレクトリ名:** `ClipRefiner`

設定ファイルの解析に失敗した場合、元ファイルは `config.toml.bak` として退避され、デフォルト設定で起動します。起動時にデスクトップ通知 (常駐モード) または標準エラー出力 (ワンショット) で案内されます。TOML 形式のため `#` でコメントを書けます。初回保存時は各項目の説明コメントが付与され、以降の保存ではユーザーが追記したコメントを維持したまま値のみ更新されます。設定ディレクトリとログファイルは、現在ユーザー専用 DACL で保護されます。

登録クリップの本文・画像は `config.toml` とは別に、同じ設定ディレクトリ内の暗号化ファイル (`registered-clips.dat` / `registered-images/`) へ保存される。設定ウィンドウのサイズ・位置は `settings-window.json` に保存される。

---

## 処理の制限

| 対象 | 上限 |
| :--- | :--- |
| クリップボード本文 | 2 MiB |
| JSON / YAML / Markdown パーサー入力 | 1 MiB |
| 正規表現パターン | 8 KiB |
| 登録クリップ | 100 件 (ラベル 64 文字) |
| お気に入り変換モード | 20 件 |
| 加工パイプライン | 10 段 |

上限を超える入力は処理されず、登録クリップの追加は拒否されます。通知・履歴メニュー・登録クリッププレビューでは、API キー・JWT・PEM 秘密鍵・資格情報行など機密らしい内容を `[機密情報のため非表示]` に自動置換します (クリップボード本体は加工対象のまま保持)。

---

## 設定項目

`mode` などの加工モード名は CLI の kebab-case (`url-decode` など) ではなく、設定ファイルでは PascalCase (`UrlDecode` など) を指定します。CLI 名との対応は [加工モード一覧](#加工モード一覧) を参照してください。

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--------- | :--- |
| `version` | number | `2` | 設定スキーマのバージョン |
| `mode` | string | `"UrlDecode"` | 使用する加工モード (`pipeline` が空の場合に監視で適用) |
| `pipeline` | array | (空) | 監視時に順に適用する加工モードの連鎖 (PascalCase の配列、最大 10 段。空の場合は `mode` のみ。例: `["UrlDecode", "Trim"]`) |
| `favorite_modes` | array | (空) | お気に入り変換モード (PascalCase の配列、最大 20 件。例: `["UrlDecode", "Trim"]`) |
| `interval_ms` | number | `1000` | クリップボードのポーリング間隔 (ミリ秒、100〜60000)。トレイメニューと設定ウィンドウでは 500 / 1000 / 2000 / 5000 の 4 択 |
| `monitor_mode` | string | `"Polling"` | 監視方式。`"Polling"` または `"Event"` |
| `is_paused` | bool | `false` | 監視を一時停止するかどうか |
| `history_enabled` | bool | `false` | 加工履歴の有効・無効 |
| `continuous_record_enabled` | bool | `false` | 連続記録モード (監視時は加工せず元テキストを記録) |
| `continuous_record_limit` | number | `10` | 連続記録の最大保持件数 (1〜100) |
| `history_limit` | number | `10` | 履歴の最大保持件数 (1〜100) |
| `notification_settings.enabled` | bool | `false` | デスクトップ通知の有効・無効 (エラー通知は常に表示) |
| `notification_settings.notify_process` | bool | `true` | 加工完了時の通知 |
| `notification_settings.notify_pause` | bool | `true` | 一時停止切替時の通知 |
| `notification_settings.notify_history_restore` | bool | `true` | 履歴復元時の通知 |
| `notification_settings.notify_ocr` | bool | `true` | OCR 完了時の通知 |
| `notification_settings.notify_registered_clip` | bool | `true` | 登録クリップ操作時の通知 |
| `notification_settings.notify_favorites` | bool | `true` | お気に入り操作時の通知 |
| `notification_settings.notify_continuous_record` | bool | `true` | 連続記録操作時の通知 |
| `notification_settings.notify_result` | bool | `false` | 通知にクリップボードの内容を表示するかどうか |
| `hotkeys.quick_selector` | string | `"Alt+Shift+S"` | クイックセレクター表示 |
| `hotkeys.clip_selector` | string | `"Alt+Shift+T"` | 登録クリップセレクター表示 |
| `hotkeys.ocr` | string | `"Alt+Shift+O"` | 画面範囲選択 OCR の開始 |
| `hotkeys.notification` | string | `"Alt+Shift+N"` | デスクトップ通知のマスター ON/OFF (エラー通知は常に表示) |
| `hotkeys.pause` | string | `"Alt+Shift+P"` | 監視の一時停止・再開 |
| `hotkeys.continuous_record` | string | `"Alt+Shift+C"` | 連続記録の有効・無効切替 |
| `hotkeys.undo` | string | `"Alt+Shift+Z"` | 直近の加工を取り消し |
| `hotkeys.quit` | string | `"Alt+Shift+Q"` | アプリケーション終了 |
| `hotkeys.favorite_mode_slots` | array | (空) | お気に入り変換モード用ホットキー (登録順インデックスに対応。未指定スロットは Alt+Shift+1〜9 / F1〜F11。空文字で無効) |
| `regex.pattern` | string | `""` | 正規表現パターン (最大 8 KiB) |
| `regex.replacement` | string | `""` | 置換文字列 (`regex-replace` で使用。`$1` 形式のキャプチャ参照可) |
| `regex.case_insensitive` | bool | `false` | 大文字小文字を無視 (`(?i)` 相当) |
| `regex.multiline` | bool | `false` | 複数行モード (`(?m)` 相当) |
| (登録クリップ) | — | — | `config.toml` には含まれない。設定ディレクトリ内の `registered-clips.dat` (暗号化) と `registered-images/` に保存。最大 100 件 |

---

## 加工モード一覧

`--mode` に渡す値 (CLI 名) と、トレイメニュー・クイックセレクタでの表示。**全 42 モード**に対応。

> **設定ファイル:** CLI は kebab-case (`url-decode` など) だが、`config.toml` では PascalCase (`UrlDecode` など) を指定する。初回保存時に生成される `config.toml` を参照するとよい。

| カテゴリ | モード名 (`--mode`) | 説明 |
| :------- | :------------------ | :--- |
| **URL 操作** | `url-encode` / `url-decode` | URL のエンコード・デコード |
| | `remove-utm` | URL から `utm_*` 計測パラメータを削除 |
| **パス操作** | `extract-basename` / `extract-basename-quoted` | パスからファイル名のみを抽出 (引用符付きオプションあり) |
| | `add-path-quotes` / `remove-path-quotes` | パスへの引用符 (`"`) の付与・削除 |
| | `path-to-slash` / `path-to-backslash` | パス区切り文字をスラッシュ / バックスラッシュに変換 |
| **行操作** | `sort-lines-asc` / `sort-lines-desc` | 行単位での昇順・降順ソート (CSV 対応) |
| | `remove-empty-lines` | 空行を削除 |
| | `remove-duplicate-lines` | 重複行を削除 |
| **トリム** | `trim` | テキスト全体の前後の空白・改行を削除 |
| | `trim-lines` | 行ごとに前後の空白を削除 |
| **エスケープ** | `escape` / `unescape` | バックスラッシュエスケープの付与・解除 |
| | `regex-escape` / `regex-unescape` | 正規表現メタ文字のエスケープ・解除 |
| **正規表現** | `regex-replace` | パターンに一致する部分を置換 (`[regex]` 設定を使用) |
| | `regex-extract` | パターンに一致する部分を行単位で抽出 |
| | `regex-delete` | パターンに一致する部分を削除 |
| | `regex-split` | パターンで分割し改行で結合 |
| **JSON 整形** | `json-format` | JSON をインデント整形 (キー順序不定) |
| | `json-format-preserve-order` | JSON をインデント整形 (キー順序保持) |
| **JSON へ変換** | `yaml-to-json` | YAML を JSON へ変換 (キー順序不定) |
| | `yaml-to-json-preserve-order` | YAML を JSON へ変換 (キー順序保持) |
| **YAML へ変換** | `json-to-yaml` | JSON を YAML へ変換 (キー順序不定) |
| | `json-to-yaml-preserve-order` | JSON を YAML へ変換 (キー順序保持) |
| **Markdown** | `markdown-to-html` | Markdown を HTML へ変換 |
| **Excel** | `excel-to-markdown` | Excel コピーデータを Markdown テーブルへ変換 |
| | `markdown-to-excel` | Markdown 表を Excel (TSV) 形式へ変換 |
| | `excel-to-image` | Excel コピーデータの見た目を画像としてクリップボードへ保存 |
| **日時変換** | `timestamp-to-datetime` / `datetime-to-timestamp` | Unix タイムスタンプ ↔ 日時文字列の変換 |
| **数値変換** | `add-comma` / `remove-comma` | 数値への 3 桁カンマ区切り付与・削除 |
| **ケース変換** | `to-camel-case` / `to-snake-case` / `to-pascal-case` / `to-kebab-case` / `to-screaming-snake-case` | 識別子のケース変換 |

正規表現モードは `[regex]` セクションでパターン・置換文字列・オプションを設定する。

---

## 加工モードの使用例

<details>
<summary><strong>UTM パラメータの削除</strong> (<code>remove-utm</code>)</summary>
<br>

| | |
| :-- | :-- |
| **入力** | `https://example.com/page?id=123&utm_source=twitter&utm_medium=social` |
| **出力** | `https://example.com/page?id=123` |

</details>

<details>
<summary><strong>Excel から Markdown へ</strong> (<code>excel-to-markdown</code>)</summary>
<br>

**入力 (TSV):**

```
ID	Name	Price
1	Apple	150
2	Banana	100
```

**出力:**

```markdown
| ID  | Name   | Price |
| --- | ------ | ----- |
| 1   | Apple  | 150   |
| 2   | Banana | 100   |
```

</details>

<details>
<summary><strong>正規表現置換</strong> (<code>regex-replace</code>)</summary>
<br>

**設定 (`config.toml`):**

```toml
[regex]
pattern = "(\\d{4})-(\\d{2})-(\\d{2})"
replacement = "$1/$2/$3"
case_insensitive = false
multiline = false
```

| | |
| :-- | :-- |
| **入力** | `会議日: 2024-01-15、締切: 2024-02-28` |
| **出力** | `会議日: 2024/01/15、締切: 2024/02/28` |

</details>

<details>
<summary><strong>タイムスタンプ変換</strong> (<code>timestamp-to-datetime</code>)</summary>
<br>

| | |
| :-- | :-- |
| **入力** | `1700000000` |
| **出力** | `2023-11-14 22:13:20` |

</details>

<details>
<summary><strong>ケース変換</strong> (<code>to-snake-case</code>)</summary>
<br>

| | |
| :-- | :-- |
| **入力** | `HelloWorld` |
| **出力** | `hello_world` |

</details>

<details>
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

| | |
| :-- | :-- |
| **入力** | `1234567` |
| **出力** | `1,234,567` |

</details>

---

## ホットキー形式

`Alt+Shift+S` のように、`+` 区切りで修飾キーとキーを指定します。

- **修飾キー**: `Alt`, `Shift`, `Ctrl` (`Control` 可), `Meta` (`Super` / `Win` 可)
- **キー**: `A`〜`Z`, `F1`〜`F12`

不正な値は読み込み時にデフォルトへ置き換えられます (お気に入りスロットの空文字は意図的な無効化として維持)。

---

## 通知 (`notification_settings`)

デスクトップ通知は **マスター** (`notification_settings.enabled`) と **種類別トグル** で制御する。起動失敗・設定読み込み失敗・クリップボード初期化失敗などの **エラー通知はマスター OFF でも常に表示** する。

| 操作 | 動作 |
| :--- | :--- |
| トレイ「通知」サブメニュー | マスター ON/OFF と種類別トグルの切替 |
| 設定ウィンドウ「通知」タブ | マスターと個別項目を GUI で編集 (マスター OFF 時も個別設定は変更可能) |
| `Alt+Shift+N` | マスター ON/OFF のみ切替 (`hotkeys.notification`) |

種類別トグルと対象操作:

| キー | 既定 | 対象 |
| :--- | :--- | :--- |
| `notify_process` | `true` | 監視モードでの加工完了 |
| `notify_pause` | `true` | 一時停止・再開 |
| `notify_history_restore` | `true` | 履歴からの復元 |
| `notify_ocr` | `true` | 画面 OCR 完了 |
| `notify_registered_clip` | `true` | 登録クリップのコピー・登録 |
| `notify_favorites` | `true` | お気に入り変換の切替 |
| `notify_continuous_record` | `true` | 連続記録の有効化・取り出しなど |
| `notify_result` | `false` | 上記通知本文にクリップボード内容を含めるか (マスターとは独立) |

`notify_result` が `true` のときも、機密らしい内容は `[機密情報のため非表示]` にマスクする。加工パイプライン適用時の加工完了通知は、連鎖したモード名を連結して表示する ([加工パイプライン](#加工パイプライン-pipeline) 参照)。

**設定例:**

```toml
[notification_settings]
enabled = true
notify_process = true
notify_pause = false
notify_history_restore = true
notify_ocr = true
notify_registered_clip = true
notify_favorites = true
notify_continuous_record = true
notify_result = false
```

**旧設定からの移行:** v0.12.1 以前の `notify_mode` / `notify_actions` は読み込み時に新項目へ自動移行し、次回保存時に TOML から除去される。

| 旧キー | 移行先 |
| :----- | :----- |
| `notify_mode = false` かつ `notify_result` 未指定 | `notify_process = false` |
| `notify_actions = false` | `notify_history_restore` / `notify_ocr` / `notify_registered_clip` / `notify_favorites` / `notify_continuous_record` を `false` |

---

## UI 操作

### システムトレイメニュー

| メニュー | 内容 |
| :------- | :--- |
| **変換モード** | カテゴリ別サブメニューから選択。お気に入りの登録・解除・即切替も可能。`pipeline` 設定時は連鎖中の全モードにチェック。連続記録 ON 時は無効化 |
| **監視方式** | `ポーリング` / `イベント` を切り替え |
| **監視周期** | `0.5秒` / `1秒` / `2秒` / `5秒` (イベント方式では無効) |
| **履歴** | 有効化・クリア、過去の加工結果の呼び出し |
| **連続記録** | 有効化・クリア、記録済みテキストの取り出し (FIFO) |
| **登録クリップ** | 登録済み文字列の呼び出し、クリップボードからの新規登録 |
| **通知** | マスター ON/OFF、種類別トグル、クリップボード内容表示。詳細は [通知 (`notification_settings`)](#通知-notification_settings) |
| **設定を開く** | [設定ウィンドウ](#設定ウィンドウ) を表示 |
| **クリップボード監視を再開** | ワーカー初期化失敗などで監視が停止したときに手動復旧 (通常は無効) |
| **一時停止 / 終了** | 監視の一時停止・再開、アプリ終了 |

### クイックセレクタ (`Alt+Shift+S`)

- **検索:** モード名・カテゴリ・CLI 名の部分一致 (ハイライト表示)
- **お気に入り:** 星印または `Ctrl+D` で登録・解除。`Ctrl+Shift+↑/↓` で並び替え
- **キー:** `↑/↓` 移動、`Enter` 決定、`Esc` クリアまたは閉じる

### 登録クリップセレクタ (`Alt+Shift+T`)

- **登録:** トレイ「登録クリップ → クリップボードを登録」、またはセレクタ内 `Ctrl+Enter` (画像優先)
- **上限:** 最大 100 件、ラベル 64 文字、テキスト 2 MiB、画像 PNG 16 MiB / 8192 px
- **キー:** `Enter` でコピー、`Del` で削除、`Esc` クリアまたは閉じる

登録クリップは `registered-clips.dat` (暗号化) と `registered-images/` に保存される (`config.toml` には含まれない)。

### 連続記録 (`Alt+Shift+C`)

- **有効化:** トレイ「連続記録 → 連続記録を有効にする」、または `Alt+Shift+C`
- **記録:** 監視時にコピーした元テキストを加工せずキュー末尾へ追加 (有効化直後は現在のクリップボード内容を先頭に積む)
- **自動加工停止:** 有効中はコピー監視による自動加工を行わない。トレイの「変換モード」メニューは無効化され、連続記録サブメニュー名は「連続記録 (自動加工停止)」、ツールチップは「ClipRefiner (連続記録中・自動加工停止)」と表示される
- **取り出し:** 連続記録 ON 時の `Ctrl+V` でキュー先頭をクリップボードへ載せてから貼り付け、またはトレイメニューから項目を選択して取り出し (除去)
- **クリア:** トレイ「連続記録 → 記録をクリア」
- **上限:** 最大 `continuous_record_limit` 件 (1〜100、デフォルト 10)。超過時は先頭から破棄。同一内容の再コピーでも重複排除せず積む
- **手動加工:** お気に入りホットキーやクイックセレクタなど、明示的な手動加工は従来どおり動作する

### お気に入り変換モード

- **登録:** トレイ「変換モード → お気に入り」またはクイックセレクタ `Ctrl+D`
- **上限:** 最大 20 件 (`favorite_modes` に永続化)
- **ホットキー:** 登録順に `Alt+Shift+1`〜`9` / `F1`〜`F11`。`hotkeys.favorite_mode_slots` でカスタマイズ (空文字で無効)

```toml
favorite_modes = ["UrlDecode", "Trim", "JsonFormat"]

[hotkeys]
favorite_mode_slots = ["", "Alt+Ctrl+J"]  # 2 件目のみカスタム、1 件目は無効
```

### 加工の取り消し (`Alt+Shift+Z`)

監視モード常駐時、加工成功直後の直近 1 件のみ取り消し可能。ワンショット (`--mode`) では利用不可。新しい加工が成功すると対象は上書きされる。

---

## 設定ウィンドウ

トレイ「設定を開く」で WebView ベースの設定ウィンドウを表示する。変更は約 200 ms の debounce 後に自動で `config.toml` へ保存され、トレイメニュー・監視ループへ即時反映される。ウィンドウを閉じる直前に保留中の変更は自動送信される。適用に失敗した場合はウィンドウ内にエラーを表示し、ウィンドウ非表示後もデスクトップ通知で伝える。サイズと位置は設定ディレクトリ内の `settings-window.json` に保存される。

ヘッダーの **config.toml** ボタン、または「ショートカット」タブから設定ファイルを既定エディタで開ける。ホットキー割り当ては `config.toml` の `[hotkeys]` で編集する (設定ウィンドウでは一覧表示のみ)。

| タブ | 内容 |
| :--- | :--- |
| **一般** | ログイン時の自動起動 |
| **お気に入り** | お気に入り変換モードの追加・並び替え・削除 (最大 20 件) |
| **正規表現** | `[regex]` のパターン・置換文字列・オプション。Rust 正規表現エンジンによるライブプレビュー付き |
| **履歴・連続記録** | 有効化・保持件数・クリア。連続記録 ON 時はバナーで自動加工停止を表示 |
| **登録クリップ** | 一覧表示・コピー・削除・クリップボードからの新規登録 |
| **監視** | 一時停止、監視方式・周期 (0.5 / 1 / 2 / 5 秒)、デフォルトモード、加工パイプラインの追加・並び替え・削除。連続記録 ON 時は加工設定セクションを無効表示 |
| **通知** | マスター ON/OFF、加工完了・一時停止・履歴復元・OCR・登録クリップ・お気に入り・連続記録の個別トグル、クリップボード内容表示 (`notify_result`) |
| **ショートカット** | 現在のグローバルホットキー一覧 (読み取り専用) |

**正規表現タブ:** パターン入力中はクライアント側でバイト長を検証し、構文チェックと `regex-replace` の置換プレビューはバックエンド (本番と同じ Rust エンジン、固定サンプル `Hello world 123`) で実行する。コンパイルできないパターンは保存されない。履歴・連続記録・登録クリップの削除は確認ダイアログを表示する。

**監視タブのデフォルトモード:** `pipeline` が空のときは従来どおり監視で適用するモードを切り替える。`pipeline` が設定済みのときは `mode` のみ更新し、パイプライン自体は維持する (監視で適用されるのは `pipeline` の連鎖)。パイプラインをクリアすると、保存済みの `mode` が再び適用される。

---

## 画面 OCR

`Alt+Shift+O` で全画面オーバーレイを表示し、ドラッグで範囲選択。確定後にキャプチャと OCR を実行し、結果をクリップボードへ書き込む。`Esc` または再度ホットキーでキャンセル。

OCR エンジンは `Windows.Media.Ocr` (日本語言語パック優先)。範囲選択は Win32 ネイティブオーバーレイ。

- **小さい選択範囲:** 短辺が小さい画像は自動拡大してから OCR
- **日本語の空白:** OCR が挿入する不要スペースを除去 (英単語間は維持)

---

## 加工パイプライン (`pipeline`)

`pipeline` に複数の加工モードを指定すると、クリップボード監視時に左から順に連鎖適用されます。空の場合は従来どおり `mode` のみが適用されます。`pipeline` が非空のときに `mode` だけを変更しても監視で適用される連鎖は変わらない (次に `pipeline` をクリアしたときのデフォルトとして `mode` が使われる)。トレイメニューからモードを選択すると `pipeline` はクリアされ、単一モードへ切り替わります。[設定ウィンドウ](#設定ウィンドウ) の監視タブでも同様にパイプラインとデフォルトモードを編集できる。

```toml
mode = "UrlDecode"
pipeline = ["UrlDecode", "Trim", "JsonFormat"]
```

- **最大段数**: 10 段まで
- **画像出力**: `ExcelToImage` はパイプライン末尾へ自動移動
- **通知**: 連鎖適用時は `URLデコード → 全体をトリム → JSON整形` のようにモード名を連結して表示

ワンショット実行時は CLI の `--pipeline` でも同様に連鎖指定できます (`--mode` より優先)。例:

```bash
./ClipRefiner.exe --pipeline url-decode trim json-format
```

常駐中は監視ループが約 2 秒ごとに設定ファイルの更新時刻を確認し、外部編集を検知したら自動で再読み込みします (アプリ自身の保存直後は誤検知を避けるため約 2 秒間は抑制)。`config.toml` を手動編集した場合は、通常 2 秒以内にホットキーなどへ反映されます。再読み込み時に TOML の解析に失敗した場合、起動時と異なりデフォルト設定へのフォールバックは行わず、通知でエラーを表示します。

---

## 監視方式 (`monitor_mode`)

| 方式 | 説明 |
| :--- | :--- |
| `Polling` | 一定間隔 (`interval_ms`) でクリップボードの内容を読み取り、変更を検知する基本方式 |
| `Event` | Windows のシーケンス番号を監視。本文の定期読み取りを避けるため、ポーリングより低遅延かつ低 CPU 負荷 |

---

## 連続記録

履歴とは独立した FIFO キュー。トレイ「連続記録 → 連続記録を有効にする」または `Alt+Shift+C` で有効化 (`continuous_record_enabled`、デフォルト無効)。監視時は加工を行わずコピーした元テキストを専用キューへ末尾追加する。有効中はコピー監視による自動加工が停止し、トレイの変換モードメニューは無効化される。最大 `continuous_record_limit` 件 (1〜100、デフォルト 10) を保持し、上限超過時は先頭から破棄する。同一内容の再コピーでも重複排除せず積む。

| 操作 | 動作 |
| :--- | :--- |
| コピー (監視) | 元テキストをキュー末尾へ追加 |
| `Ctrl+V` 貼り付け | 連続記録 ON 時、キュー先頭をクリップボードへ載せてから通常の貼り付けを実行 (キューが空のときは従来どおり) |
| トレイメニュー項目選択 | 指定位置のエントリをクリップボードへ取り出し (除去) |
| 有効化直後 | 現在のクリップボード内容をキューへ積む (`CatchUpContinuousRecord`) |

```toml
continuous_record_enabled = true
continuous_record_limit = 10

[hotkeys]
continuous_record = "Alt+Shift+C"
```

ホットキーやメニューからの手動加工は従来どおり動作する。トレイメニューとホットキーの操作は [UI 操作](#ui-操作) を参照。

---

## クリップボード履歴のセキュリティ

履歴はトレイ「履歴 → 履歴機能を有効にする」で有効化 (`history_enabled`、デフォルト無効)。最大 `history_limit` 件 (1〜100、デフォルト 10) を保持し、同一内容は最新位置へ移動して重複しない。

連続記録も履歴と同様に**ディスクへ書き込まず**、メモリ上で ChaCha20-Poly1305 暗号化して保持する (プロセス終了で消える)。機能の違いは [連続記録](#連続記録) を参照。

履歴は機密情報を含む可能性があるため、**ディスクへ書き込まない**ことに加え、**メモリ上でも平文を常駐させない**よう多層的に保護している。

| 対策 | 内容 |
| :--- | :--- |
| **ディスク非永続化** | 履歴・連続記録はファイル・DB へ書き込まず、プロセス実行中のメモリ上のみ。再起動で消える |
| **メモリ内暗号化** | 起動ごとに生成したセッション鍵 (32 バイト) で各エントリを ChaCha20-Poly1305 暗号化。エントリごとにランダムノンス |
| **平文の非保持** | 履歴の重複判定は BLAKE3 ハッシュのみ。履歴・連続記録ストア内に本文の平文は格納しない |
| **復号時のゼロ化** | 呼び出し・取り消しで復号した文字列は `SecretString` で保持し、スコープ終了時にゼロ化 |
| **終了時の破棄** | プロセス終了時に暗号鍵と暗号文バッファをクリア |
| **表示マスキング** | 履歴メニューのラベルで API キー・JWT 等を `[機密情報のため非表示]` に置換 |

加工の取り消し用に保持する直近 1 件の加工前テキストも、同様にメモリ上のみ・ゼロ化付きで管理する。

> **補足:** OS のメモリスワップや他プロセスからのメモリダンプなど、ランタイム環境に依存するリスクまでは完全には防げない。機密性の高い内容は履歴をオフにするか、使用後に「履歴をクリア」することを推奨する。

---

## ログイン時の自動起動

設定ウィンドウ「一般」タブから切り替える。現在のユーザー向けレジストリ `Run` キーに登録する。

実行ファイルのパスが変わった場合 (再インストールや移動後など) は、一度オフにしてから再度オンにすると正しいパスで再登録される。

---

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存されます。不具合報告時は該当する日付のログファイルを添付してください。

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\logs\` |

ログレベルの詳細設定 (`RUST_LOG` など) は [DEVELOPMENT.md のログ節](DEVELOPMENT.md#ログ) を参照してください。
