# ClipRefiner 設定リファレンス

ユーザ向けの機能説明・操作方法は [README.md](README.md) を参照してください。ビルドや開発手順は [DEVELOPMENT.md](DEVELOPMENT.md) を参照してください。

## 目次

- [設定ファイルの場所](#設定ファイルの場所)
- [処理の制限](#処理の制限)
- [設定項目](#設定項目)
- [ホットキー形式](#ホットキー形式)
- [加工パイプライン (`pipeline`)](#加工パイプライン-pipeline)
- [監視方式 (`monitor_mode`)](#監視方式-monitor_mode)
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

`mode` などの加工モード名は CLI の kebab-case (`url-decode` など) ではなく、設定ファイルでは PascalCase (`UrlDecode` など) を指定します。CLI 名との対応は [README の加工モード一覧](README.md#️-加工モード一覧) を参照してください。

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
| `[[clips]]` | array | (空) | 登録クリップ (`label` / `text` / `image_file`)。最大 100 件 |

---

## ホットキー形式

`Alt+Shift+S` のように、`+` 区切りで修飾キーとキーを指定します。

- **修飾キー**: `Alt`, `Shift`, `Ctrl` (`Control` 可), `Meta` (`Super` / `Win` 可)
- **キー**: `A`〜`Z`, `F1`〜`F12`

不正な値は読み込み時にデフォルトへ置き換えられます (お気に入りスロットの空文字は意図的な無効化として維持)。

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

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存されます。不具合報告時は該当する日付のログファイルを添付してください。

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\logs\` |
| **Linux / macOS** | `~/.config/clip-refiner/logs/` |

ログレベルの詳細設定 (`RUST_LOG` など) は [DEVELOPMENT.md のログ節](DEVELOPMENT.md#ログ) を参照してください。
