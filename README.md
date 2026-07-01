<div align="center">

<img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">

<h1>ClipRefiner</h1>

<p>
  <strong>クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工するデスクトップツール</strong>
</p>

<p>
  <img src="https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPldpbmRvd3M8L3RpdGxlPjxwYXRoIGQ9Ik0zIDEyLjVWNi44bDgtMS4xdjYuOEgzem05LTcuMyAxMC0xLjR2OC43SDEyVjUuMnpNMyAxMy41aDh2NS43bC04LTEuMnYtNC41em05IDBoMTB2OC42bC0xMC0xLjR2LTcuMnoiLz48L3N2Zz4%3D&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux">
  <img src="https://img.shields.io/badge/License-All%20Rights%20Reserved-F59E0B?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPkNvcHlyaWdodDwvdGl0bGU%2BPHBhdGggZD0iTTEyIDJhMTAgMTAgMCAxIDAgMCAyMCAxMCAxMCAwIDAgMCAwLTIwem0wIDJhOCA4IDAgMSAxIDAgMTYgOCA4IDAgMCAxIDAtMTZ6bS0xIDQuNWMtMi4yIDAtMy41IDEuNi0zLjUgMy41czEuMyAzLjUgMy41IDMuNWMxLjEgMCAyLS41IDIuNi0xLjJsLTEuMi0xLjJjLS40LjUtMSAuOC0xLjYuOC0xLjIgMC0yLS45LTItMnMuOC0yIDItMmMuNiAwIDEuMS4yIDEuNS42bDEuMi0xLjJjLS43LS43LTEuNy0xLjEtMi45LTEuMXoiLz48L3N2Zz4%3D&logoColor=white" alt="License">
</p>

<p>
  <sub>42 種類の加工モード &middot; 加工パイプライン &middot; お気に入り &middot; 画面 OCR &middot; 暗号化履歴 &middot; 機密情報マスキング</sub>
</p>

</div>

---

## ドキュメント

| ドキュメント | 内容 |
| :----------- | :--- |
| **README.md** (このファイル) | 概要・クイックスタート・基本操作 |
| [CONFIG.md](CONFIG.md) | 設定リファレンス・加工モード一覧・ホットキー・使用例・セキュリティ |
| [CHANGELOG.md](CHANGELOG.md) | バージョンごとの変更履歴 |
| [DEVELOPMENT.md](DEVELOPMENT.md) | ビルド・テスト・開発者向け情報 |

---

## クイックスタート

**監視モード (常駐):** 引数なしで起動するとシステムトレイに常駐し、クリップボードの変更を自動加工する。

```bash
./ClipRefiner.exe          # Windows
./ClipRefiner              # macOS / Linux
```

**ワンショット:** `--mode` (`-m`) で 1 回だけ加工して終了。複数段は `--pipeline` を使う。

```bash
./ClipRefiner.exe -m url-decode
./ClipRefiner.exe --pipeline url-decode trim
```

加工モード名は CLI では kebab-case (`url-decode`)、`config.toml` では PascalCase (`UrlDecode`)。[CONFIG.md の加工モード一覧](CONFIG.md#加工モード一覧) を参照。

---

## 主な機能

| 機能 | 概要 |
| :--- | :--- |
| **監視モード** | トレイ常駐でクリップボード変更を検知し自動加工 |
| **ワンショット** | CLI から 1 回だけ加工 |
| **加工パイプライン** | 最大 10 段のモード連鎖 (`pipeline` / `--pipeline`) |
| **クイックセレクタ** | コマンドパレット風 UI でモード検索・切替 (`Alt+Shift+S`) |
| **登録クリップ** | よく使うテキスト・画像を暗号化保存し即コピー (`Alt+Shift+T`) |
| **お気に入り変換** | よく使うモードを専用ホットキー (最大 20 件) で即切替 |
| **画面 OCR** | 範囲選択して OS 標準 OCR でクリップボードへ (`Alt+Shift+O`) |
| **正規表現モード** | `config.toml` の `[regex]` で置換・抽出・削除・分割 |
| **履歴** | 加工結果をメモリ上のみ暗号化保持 (オプション) |
| **加工の取り消し** | 直近 1 件を元テキストへ復元 (`Alt+Shift+Z`) |
| **機密マスキング** | 通知・履歴・登録クリッププレビューで API キー等を非表示 |

---

## 使用方法

### 監視モード

トレイアイコンの右クリックメニューから加工モード・監視方式・履歴などを操作する。`config.toml` の `pipeline` で複数モードを連鎖適用できる。監視方式 (`Polling` / `Event`) の違いは [CONFIG.md](CONFIG.md#監視方式-monitor_mode) を参照。

### ワンショットモード

```bash
./ClipRefiner.exe -m json-format
./ClipRefiner.exe -m regex-replace --regex-pattern "(\d{4})-(\d{2})-(\d{2})" --regex-replacement "$1/$2/$3"
```

正規表現オプションはワンショット時のみ有効。常駐時は `config.toml` の `[regex]` を使用する。

### コマンドラインオプション

| オプション | 説明 |
| :--------- | :--- |
| `-m`, `--mode <MODE>` | ワンショットで実行する加工モード |
| `--pipeline <MODE>...` | 順に適用するモード列 (`--mode` より優先) |
| `--regex-pattern`, `--regex-replacement` | 正規表現の上書き (ワンショット時) |
| `--regex-case-insensitive`, `--regex-multiline` | 正規表現フラグ |
| `-h`, `--help` / `-V`, `--version` | ヘルプ / バージョン |

---

## UI と操作

### システムトレイ

変換モード・監視方式/周期・履歴・登録クリップ・通知・設定の開閉・再読み込み・一時停止・終了などを提供する。メニュー項目の詳細は [CONFIG.md の UI 操作](CONFIG.md#ui-操作) を参照。

### クイックセレクタ / 登録クリップセレクタ

いずれもコマンドパレット風 UI。モード名・カテゴリ・CLI 名で検索し、`Enter` で決定、`Esc` で閉じる。クイックセレクタでは `Ctrl+D` でお気に入り登録、`Ctrl+Shift+↑/↓` で並び替え。登録クリップセレクタでは `Ctrl+Enter` でクリップボード内容を新規登録、`Del` で削除。

### お気に入り変換モード

トレイ「変換モード → お気に入り」またはクイックセレクタから登録。登録順に `Alt+Shift+1`〜`9` / `F1`〜`F11` が割り当てられ、押下で即切替・加工する。

### 画面 OCR

`Alt+Shift+O` で全画面オーバーレイを表示し、ドラッグで範囲選択。OS 標準 OCR で認識したテキストをクリップボードへ書き込む。Linux では Tesseract と日本語言語パックが必要。詳細は [CONFIG.md の画面 OCR](CONFIG.md#画面-ocr) を参照。

---

## グローバルホットキー (既定)

`config.toml` の `hotkeys` で変更可能。編集後は自動反映、またはトレイ「設定を再読み込み」で即時反映。

| ホットキー | 動作 |
| :--------- | :--- |
| `Alt+Shift+S` | クイックセレクタ |
| `Alt+Shift+T` | 登録クリップセレクタ |
| `Alt+Shift+O` | 画面 OCR |
| `Alt+Shift+P` | 監視の一時停止 / 再開 |
| `Alt+Shift+Z` | 加工の取り消し |
| `Alt+Shift+N` | 通知 ON/OFF |
| `Alt+Shift+Q` | 終了 |
| `Alt+Shift+1`〜`9`, `F1`〜`F11` | お気に入り変換 (登録順) |

形式の詳細は [CONFIG.md のホットキー形式](CONFIG.md#ホットキー形式) を参照。

---

## 履歴・セキュリティ

履歴機能はトレイ「履歴」から有効化できる (デフォルト無効)。加工結果を最大 `history_limit` 件 (既定 10) 保持し、トレイから呼び出せる。

履歴・登録クリップ・加工取り消し用テキストは**ディスクへ書かず**、メモリ上で ChaCha20-Poly1305 暗号化して保持する。通知や履歴ラベルでは機密らしい内容を自動マスクする。多層的な保護の詳細と利用上の注意は [CONFIG.md のセキュリティ](CONFIG.md#クリップボード履歴のセキュリティ) を参照。

---

## インストール

リリース済みパッケージがある場合:

| プラットフォーム | 方法 |
| :--------------- | :--- |
| **Windows** | MSI を実行 (per-user、管理者権限不要) |
| **macOS** | DMG から `ClipRefiner.app` を Applications へ |
| **Linux** | `sudo dpkg -i clip-refiner_{version}-1_{arch}.deb` |

ソースからのビルド・パッケージ作成は [DEVELOPMENT.md](DEVELOPMENT.md) を参照。実行ファイル単体のポータブル利用も可能 (設定はユーザの設定ディレクトリへ保存)。

**ログイン時の自動起動:** トレイ「ログイン時に起動」から切替 (Windows: レジストリ Run、macOS: LaunchAgent、Linux: XDG autostart)。実行ファイルを移動した場合は一度オフにしてから再度オンにする。

---

## 設定

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\config.toml` |
| **Linux / macOS** | `~/.config/clip-refiner/config.toml` |

設定項目・加工パイプライン・監視方式・処理上限・ログの場所・加工モード一覧・使用例は **[CONFIG.md](CONFIG.md)** を参照。

---

<div align="center">

## ライセンス

[All Rights Reserved](LICENSE)

</div>
