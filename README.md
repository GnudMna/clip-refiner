<div align="center">
  <img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">
  <h1 align="center">ClipRefiner</h1>
  <p>クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工するツール
  </p>

  [![License: All Rights Reserved](https://img.shields.io/badge/License-All%20Rights%20Reserved-yellow.svg)](LICENSE)
  [![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust)](https://www.rust-lang.org/)
  [![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)]()
</div>

---

## 📌 目次

- [主な機能](#主な機能)
- [使用方法](#使用方法)
  - [監視モード（常駐）](#監視モード常駐)
  - [ワンショットモード](#ワンショットモード)
- [グローバルホットキー](#グローバルホットキー)
- [加工モード一覧と使用例](#加工モード一覧と使用例)
- [インストール・ビルド](#インストールビルド)
- [設定](#設定)
- [ログ](#ログ)
- [ライセンス](#ライセンス)

---

## ✨ 主な機能

- 🔍 **監視モード**: システムトレイに常駐し、クリップボードの変更を検知して自動的にテキストを加工します。
- ⚡ **ワンショットモード**: コマンドラインから特定のモードを指定して、現在のクリップボード内容を一度だけ加工します。
- 🪟 **クイックセレクタ UI**: コマンドパレット風のウィンドウから加工モードをキーボードで素早く切り替えられます。
- 🔔 **デスクトップ通知**: 加工結果や一時停止の状態変化をシステム通知で確認できます。
- ⌨️ **グローバルホットキー**: アプリを問わず、どのウィンドウからでもキー操作で機能を呼び出せます。

### 🛠️ 加工モード一覧

| カテゴリ | モード名 (`--mode`) | 説明 |
| :--- | :--- | :--- |
| **URL操作** | `url-encode` / `url-decode` | URLのエンコード・デコード |
| | `remove-utm` | URLから `utm_*` 計測パラメータを削除 |
| **パス操作** | `extract-basename` / `extract-basename-quoted` | パスからファイル名のみを抽出（引用符付きオプションあり） |
| | `add-path-quotes` / `remove-path-quotes` | パスへの引用符 (`"`) の付与・削除 |
| | `path-to-slash` / `path-to-backslash` | パス区切り文字をスラッシュ/バックスラッシュに変換 |
| **行操作** | `sort-lines-asc` / `sort-lines-desc` | 行単位での昇順・降順ソート（CSV対応） |
| | `remove-empty-lines` | 空行を削除 |
| | `remove-duplicate-lines` | 重複行を削除 |
| **トリム** | `trim` | テキスト全体の前後の空白・改行を削除 |
| | `trim-lines` | 行ごとに前後の空白を削除 |
| **エスケープ** | `escape` / `unescape` | バックスラッシュエスケープの付与・解除 |
| | `regex-escape` / `regex-unescape` | 正規表現メタ文字のエスケープ・解除 |
| **JSON整形** | `json-format` | JSONをインデント整形（キー順序不定） |
| | `json-format-preserve-order` | JSONをインデント整形（キー順序保持） |
| **JSONへ変換** | `yaml-to-json` | YAMLをJSONへ変換（キー順序不定） |
| | `yaml-to-json-preserve-order` | YAMLをJSONへ変換（キー順序保持） |
| **YAMLへ変換** | `json-to-yaml` | JSONをYAMLへ変換（キー順序不定） |
| | `json-to-yaml-preserve-order` | JSONをYAMLへ変換（キー順序保持） |
| **その他** | `markdown-to-html` | MarkdownをHTMLへ変換 |
| | `excel-to-markdown` | ExcelコピーデータをMarkdownテーブルへ変換 |
| | `timestamp-to-datetime` / `datetime-to-timestamp` | Unixタイムスタンプ ↔ 日時文字列の変換 |
| | `add-comma` / `remove-comma` | 数値への3桁カンマ区切り付与・削除 |

---

## 🚀 使用方法

### 監視モード（常駐）

引数なしで実行すると、システムトレイ（通知領域）にアイコンが表示されます。
右クリックメニューから加工モードの切り替えや監視の一時停止が可能です。

```bash
./ClipRefiner.exe
```

### ワンショットモード

特定の加工を一度だけ行いたい場合に使用します。

```bash
# クリップボード内のURLをデコードする
./ClipRefiner.exe --mode url-decode
```

---

## ⌨️ グローバルホットキー

監視モード常駐時に、アクティブなウィンドウを問わず以下のホットキーが使用できます。

| ホットキー | 動作 |
| :--- | :--- |
| `Alt + Shift + S` | クイックセレクタUI の表示・非表示 |
| `Alt + Shift + P` | クリップボード監視の一時停止・再開 |
| `Alt + Shift + N` | デスクトップ通知のON/OFF切り替え |
| `Alt + Shift + Q` | アプリケーションの終了 |

---

## 📝 加工モード一覧と使用例

### UTMパラメータの削除 (`remove-utm`)
- **入力**: `https://example.com/page?id=123&utm_source=twitter&utm_medium=social`
- **出力**: `https://example.com/page?id=123`

### ExcelからMarkdownへ (`excel-to-markdown`)
- **入力 (TSV)**:
  ```
  ID	Name	Price
  1	Apple	150
  2	Banana	100
  ```
- **出力**:
  ```markdown
  | ID | Name | Price |
  | --- | --- | --- |
  | 1 | Apple | 150 |
  | 2 | Banana | 100 |
  ```

### タイムスタンプ変換 (`timestamp-to-datetime`)
- **入力**: `1700000000`
- **出力**: `2023-11-14 22:13:20`

### カンマ区切り付与 (`add-comma`)
- **入力**: `1234567`
- **出力**: `1,234,567`

---

## 🛠️ インストール・ビルド

### 前提条件

- [Rust / Cargo](https://www.rust-lang.org/tools/install)（edition 2024、Rust 1.85 以上）

#### Linux の追加パッケージ

GUIおよび通知機能のために、以下のパッケージが必要になる場合があります:

```bash
sudo apt-get install libdbus-1-dev pkg-config libatk1.0-dev libgtk-3-dev
```

### ビルド

```bash
git clone <repository_url>
cd clip-refiner
cargo build --release
```

バイナリは `target/release/ClipRefiner` (`ClipRefiner.exe` on Windows) に生成されます。

---

## ⚙️ 設定

設定ファイル（`config.json`）はアプリケーション終了時に自動保存され、以下の場所に配置されます。

- **Windows**: `%APPDATA%\ClipRefiner\config.json`
- **Linux/macOS**: `~/.config/clip-refiner/config.json`

### 設定項目

| キー | 型 | デフォルト | 説明 |
| :--- | :--- | :--- | :--- |
| `mode` | string | `"UrlDecode"` | 使用する加工モード |
| `interval_ms` | number | `1000` | クリップボードのポーリング間隔（ミリ秒） |
| `monitor_mode` | string | `"Polling"` | 監視方式。`"Polling"` または `"Event"`（Windows のみ） |
| `is_paused` | bool | `false` | 監視を一時停止するかどうか |
| `history_enabled` | bool | `false` | 加工履歴の有効・無効 |
| `notification_settings.enabled` | bool | `false` | デスクトップ通知の有効・無効 |
| `notification_settings.notify_mode` | bool | `true` | モード変更時の通知 |
| `notification_settings.notify_result` | bool | `true` | 加工結果の通知 |
| `notification_settings.notify_pause` | bool | `true` | 一時停止切替時の通知 |

> **`monitor_mode: "Event"`（Windows 専用）**: OSのクリップボード更新イベントを利用するため、ポーリングより低遅延かつ低CPU負荷です。

---

## 📋 ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存されます。

- **Windows**: `%APPDATA%\ClipRefiner\logs\`
- **Linux/macOS**: `~/.config/clip-refiner/logs/`

ログレベルは環境変数 `RUST_LOG` で制御できます（例: `RUST_LOG=debug`）。

---

## 📄 ライセンス

[All Rights Reserved](LICENSE)
