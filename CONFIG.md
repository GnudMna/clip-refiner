# ClipRefiner 設定リファレンス

ユーザ向けの概要・クイックスタートは [README.md](README.md) を参照してください。ビルドや開発手順は [DEVELOPMENT.md](DEVELOPMENT.md) を参照してください。

## 目次

- [設定ファイルの場所](#設定ファイルの場所)
- [処理の制限](#処理の制限)
- [設定項目](#設定項目)
- [加工モード一覧](#加工モード一覧)
- [加工モードの使用例](#加工モードの使用例)
- [ホットキー形式](#ホットキー形式)
- [UI 操作](#ui-操作)
- [画面 OCR](#画面-ocr)
- [加工パイプライン (`pipeline`)](#加工パイプライン-pipeline)
- [監視方式 (`monitor_mode`)](#監視方式-monitor_mode)
- [クリップボード履歴のセキュリティ](#クリップボード履歴のセキュリティ)
- [ログイン時の自動起動](#ログイン時の自動起動)
- [ログ](#ログ)

---

## 設定ファイルの場所

設定ファイル (`config.toml`) は設定変更のたびに自動保存され、以下の場所に配置されます。

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\config.toml` |
| **Linux / macOS** | `~/.config/clip-refiner/config.toml` |

**設定ディレクトリ名:** Windows は `ClipRefiner`、Linux/macOS は `clip-refiner` (OS ごとの慣例に合わせた名称)

設定ファイルの解析に失敗した場合、元ファイルは `config.toml.bak` として退避され、デフォルト設定で起動します。TOML 形式のため `#` でコメントを書けます。初回保存時は各項目の説明コメントが付与され、以降の保存ではユーザーが追記したコメントを維持したまま値のみ更新されます。設定ディレクトリとログファイルは、Unix では所有者専用パーミッション、Windows では現在ユーザー専用 DACL で保護されます。

登録クリップの本文・画像は `config.toml` とは別に、同じ設定ディレクトリ内の暗号化ファイル (`registered-clips.dat` / `registered-images/`) へ保存されます。

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
| `interval_ms` | number | `1000` | クリップボードのポーリング間隔 (ミリ秒、100〜60000) |
| `monitor_mode` | string | `"Polling"` | 監視方式。`"Polling"` または `"Event"` |
| `is_paused` | bool | `false` | 監視を一時停止するかどうか |
| `history_enabled` | bool | `false` | 加工履歴の有効・無効 |
| `history_limit` | number | `10` | 履歴の最大保持件数 (1〜100) |
| `notification_settings.enabled` | bool | `false` | デスクトップ通知の有効・無効 |
| `notification_settings.notify_mode` | bool | `true` | モード変更時の通知 |
| `notification_settings.notify_result` | bool | `false` | 通知にクリップボードの内容を表示するかどうか |
| `notification_settings.notify_pause` | bool | `true` | 一時停止切替時の通知 |
| `hotkeys.quick_selector` | string | `"Alt+Shift+S"` | クイックセレクター表示 |
| `hotkeys.clip_selector` | string | `"Alt+Shift+T"` | 登録クリップセレクター表示 |
| `hotkeys.ocr` | string | `"Alt+Shift+O"` | 画面範囲選択 OCR の開始 |
| `hotkeys.notification` | string | `"Alt+Shift+N"` | デスクトップ通知の ON/OFF |
| `hotkeys.pause` | string | `"Alt+Shift+P"` | 監視の一時停止・再開 |
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

## UI 操作

### システムトレイメニュー

| メニュー | 内容 |
| :------- | :--- |
| **変換モード** | カテゴリ別サブメニューから選択。お気に入りの登録・解除・即切替も可能。`pipeline` 設定時は連鎖中の全モードにチェック |
| **監視方式** | `ポーリング` / `イベント` を切り替え |
| **監視周期** | `0.5秒` / `1秒` / `2秒` / `5秒` (イベント方式では無効) |
| **履歴** | 有効化・クリア、過去の加工結果の呼び出し |
| **登録クリップ** | 登録済み文字列の呼び出し、クリップボードからの新規登録 |
| **通知** | デスクトップ通知の有効化と内容 (モード変更・クリップボードの内容・一時停止) の個別設定 |
| **設定を開く / 再読み込み** | `config.toml` の編集、またはディスク上の設定を再起動なしで反映 |
| **ショートカット一覧** | 現在のグローバルホットキー割り当てを通知で表示 |
| **ログイン時に起動** | OS ログイン時の自動起動を切替 |
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

## 画面 OCR

`Alt+Shift+O` で全画面オーバーレイを表示し、ドラッグで範囲選択。確定後にキャプチャと OCR を実行し、結果をクリップボードへ書き込む。`Esc` または再度ホットキーでキャンセル。

| OS | OCR エンジン | 補足 |
| :--- | :----------- | :--- |
| **Windows** | `Windows.Media.Ocr` | 日本語言語パック優先。Win32 ネイティブオーバーレイ |
| **macOS** | Apple Vision | WebView オーバーレイ。macOS 11 以降 |
| **Linux** | Tesseract | `tesseract-ocr` と `tesseract-ocr-jpn` が必要。X11 推奨 (Wayland は compositor 依存) |

- **小さい選択範囲:** 短辺が小さい画像は自動拡大してから OCR
- **日本語の空白:** OCR が挿入する不要スペースを除去 (英単語間は維持)

---

## 加工パイプライン (`pipeline`)

`pipeline` に複数の加工モードを指定すると、クリップボード監視時に左から順に連鎖適用されます。空の場合は従来どおり `mode` のみが適用されます。トレイメニューからモードを選択すると `pipeline` はクリアされ、単一モードへ切り替わります。

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

常駐中は監視ループが約 2 秒ごとに設定ファイルの更新時刻を確認し、外部編集を検知したら自動で再読み込みします (アプリ自身の保存直後は誤検知を避けるため約 2 秒間は抑制)。すぐに反映したい場合や形式の確認には、トレイの「設定を再読み込み」を使用してください。再読み込み時に TOML の解析に失敗した場合、起動時と異なりデフォルト設定へのフォールバックは行わず、通知でエラーを表示します。

---

## 監視方式 (`monitor_mode`)

| 方式 | 説明 |
| :--- | :--- |
| `Polling` | 一定間隔 (`interval_ms`) でクリップボードの内容を読み取り、変更を検知。すべてのプラットフォームで動作する基本方式 |
| `Event` | OS の変更トークン (Windows: シーケンス番号、macOS: `changeCount`、Linux: X11 の CLIPBOARD オーナー / Wayland の data-control 選択イベント) を監視。本文の定期読み取りを避けるため、ポーリングより低遅延かつ低 CPU 負荷 |

**Linux での注意:** Wayland では `ext-data-control-v1` または `wlr-data-control-unstable-v1` に対応した compositor (GNOME、KDE、Sway、Hyprland など) で `Event` 方式が利用できます。いずれのバックエンドも利用できない環境では、自動的にポーリングへフォールバックします。

---

## クリップボード履歴のセキュリティ

履歴はトレイ「履歴 → 履歴機能を有効にする」で有効化 (`history_enabled`、デフォルト無効)。最大 `history_limit` 件 (1〜100、デフォルト 10) を保持し、同一内容は最新位置へ移動して重複しない。

履歴は機密情報を含む可能性があるため、**ディスクへ書き込まない**ことに加え、**メモリ上でも平文を常駐させない**よう多層的に保護している。

| 対策 | 内容 |
| :--- | :--- |
| **ディスク非永続化** | ファイル・DB へ書き込まず、プロセス実行中のメモリ上のみ。再起動で履歴は消える |
| **メモリ内暗号化** | 起動ごとに生成したセッション鍵 (32 バイト) で各エントリを ChaCha20-Poly1305 暗号化。エントリごとにランダムノンス |
| **平文の非保持** | 重複判定は BLAKE3 ハッシュのみ。履歴ストア内に加工後テキストの平文は格納しない |
| **復号時のゼロ化** | 呼び出し・取り消しで復号した文字列は `SecretString` で保持し、スコープ終了時にゼロ化 |
| **終了時の破棄** | プロセス終了時に暗号鍵と暗号文バッファをクリア |
| **表示マスキング** | 履歴メニューのラベルで API キー・JWT 等を `[機密情報のため非表示]` に置換 |

加工の取り消し用に保持する直近 1 件の加工前テキストも、同様にメモリ上のみ・ゼロ化付きで管理する。

> **補足:** OS のメモリスワップや他プロセスからのメモリダンプなど、ランタイム環境に依存するリスクまでは完全には防げない。機密性の高い内容は履歴をオフにするか、使用後に「履歴をクリア」することを推奨する。

---

## ログイン時の自動起動

トレイ「ログイン時に起動」から切り替え。各プラットフォームのネイティブ機構を使用する。

| プラットフォーム | 仕組み |
| :--------------- | :----- |
| **Windows** | 現在のユーザー向けレジストリ `Run` キー |
| **macOS** | `~/Library/LaunchAgents/` への LaunchAgent 配置 |
| **Linux** | XDG `~/.config/autostart/` への `.desktop` ファイル配置 |

実行ファイルのパスが変わった場合 (再インストールや移動後など) は、一度オフにしてから再度オンにすると正しいパスで再登録される。

---

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存されます。不具合報告時は該当する日付のログファイルを添付してください。

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\logs\` |
| **Linux / macOS** | `~/.config/clip-refiner/logs/` |

ログレベルの詳細設定 (`RUST_LOG` など) は [DEVELOPMENT.md のログ節](DEVELOPMENT.md#ログ) を参照してください。
