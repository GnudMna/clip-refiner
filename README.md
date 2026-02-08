<p align="center">
  <img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">
  <h1 align="center">ClipRefiner</h1>
</p>

**ClipRefiner** は、クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工するデスクトップツールです。
URLのデコード、UTMパラメータの削除、JSONやYAMLの整形・変換などを、コピーするだけで即座に行うことができます。

[![License: All Rights Reserved](https://img.shields.io/badge/License-All%20Rights%20Reserved-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)]()

---

## 📌 目次

- [主な機能](#主な機能)
- [使用方法](#使用方法)
  - [監視モード（常駐）](#監視モード常駐)
  - [ワンショットモード](#ワンショットモード)
- [クイック例](#クイック例)
- [インストール・ビルド](#インストールビルド)
- [設定](#設定)
- [ライセンス](#ライセンス)

---

## ✨ 主な機能

ClipRefiner は、用途に合わせて使い分けられる2つのモードを備えています。

- 🔍 **監視モード**: システムトレイに常駐し、クリップボードの変更を検知して自動的にテキストを加工します。
- ⚡ **ワンショットモード**: コマンドラインから特定のモードを指定して、現在のクリップボード内容を一度だけ加工します。

### 🛠️ 加工モード一覧

| カテゴリ | モード名 (`--mode`) | 説明 |
| :--- | :--- | :--- |
| **URL操作** | `url-encode` / `url-decode` | URLのエンコード・デコード |
| | `remove-utm` | URLから `utm_*` パラメータを削除 |
| **パス操作** | `extract-basename` / `extract-basename-quoted` | パスからファイル名のみを抽出 |
| | `add-path-quotes` / `remove-path-quotes` | パスの引用符 (`"`) の付与・削除 |
| **テキスト整形** | `trim` / `trim-lines` | 全体または行ごとの空白・改行削除 |
| | `sort-lines-asc` / `sort-lines-desc` | 行単位での昇順・降順ソート |
| | `remove-empty-lines` / `remove-duplicate-lines` | 空行や重複行の削除 |
| **データ変換** | `json-format` / `json-format-preserve-order` | JSONの整形（順序保持オプションあり） |
| | `json-to-yaml` / `json-to-yaml-preserve-order` | JSONをYAMLへ変換（順序保持オプションあり） |
| | `yaml-to-json` / `yaml-to-json-preserve-order` | YAMLをJSONへ変換（順序保持オプションあり） |
| | `excel-to-markdown` | Excel(TSV)からMarkdownテーブルへ変換 |
| | `markdown-to-html` | MarkdownからHTMLへ変換 |
| **その他** | `timestamp-to-datetime` / `datetime-to-timestamp` | Unixスタンプ ↔ 日時文字列の変換 |
| | `add-comma` / `remove-comma` | 数値の3桁カンマ区切り付与・削除 |
| | `escape` / `unescape` | 文字列や正規表現のエスケープ操作 |

---

## 🚀 使用方法

### 監視モード（常駐）

引数なしで実行すると、システムトレイ（通知領域）にアイコンが表示されます。
右クリックメニューから、加工モードの切り替えや監視の一時停止が可能です。

```bash
# Windows
./ClipRefiner.exe

# macOS / Linux
./clip-refiner
```

### ワンショットモード

特定の加工を一度だけ行いたい場合に便利です。

```bash
# クリップボード内のURLをデコードする
./ClipRefiner.exe --mode url-decode
```

---

## 📝 クリップ例

よく使われるモードの入力・出力例です。

### UTMパラメータの削除 (`remove-utm`)
- **Input**: `https://example.com/page?id=123&utm_source=twitter&utm_medium=social`
- **Output**: `https://example.com/page?id=123`

### ExcelからMarkdownへ (`excel-to-markdown`)
- **Input (TSV)**:
  ```text
  ID	Name	Price
  1	Apple	150
  2	Banana	100
  ```
- **Output**:
  ```markdown
  | ID | Name | Price |
  | --- | --- | --- |
  | 1 | Apple | 150 |
  | 2 | Banana | 100 |
  ```

---

## 🛠️ インストール・ビルド

### 動作要件 (Linux)
GUIおよび通知機能のために、以下のパッケージが必要になる場合があります:
```bash
sudo apt-get install libdbus-1-dev pkg-config libatk1.0-dev libgtk-3-dev
```

### ビルド方法
Rustの開発環境（Cargo）が必要です。

```bash
git clone <repository_url>
cd clip-refiner
cargo build --release
```
バイナリは `target/release/` に生成されます。

---

## ⚙️ 設定

設定ファイル（`config.json`）は以下の場所に保存されます。

- **Windows**: `%APPDATA%\ClipRefiner\config.json`
- **Linux/macOS**: `~/.config/clip-refiner/config.json`

---

## 📄 ライセンス

[All Rights Reserved](LICENSE)
