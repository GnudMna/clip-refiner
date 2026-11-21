<div align="center">

<img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">

<h1>ClipRefiner</h1>

<p>
  <strong>クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工する Windows 向けデスクトップツール</strong>
</p>

<p>
  <img src="https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPldpbmRvd3M8L3RpdGxlPjxwYXRoIGQ9Ik0zIDEyLjVWNi44bDgtMS4xdjYuOEgzem05LTcuMyAxMC0xLjR2OC43SDEyVjUuMnpNMyAxMy41aDh2NS43bC04LTEuMnYtNC41em05IDBoMTB2OC42bC0xMC0xLjR2LTcuMnoiLz48L3N2Zz4%3D&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/License-All%20Rights%20Reserved-F59E0B?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPkNvcHlyaWdodDwvdGl0bGU%2BPHBhdGggZD0iTTEyIDJhMTAgMTAgMCAxIDAgMCAyMCAxMCAxMCAwIDAgMCAwLTIwem0wIDJhOCA4IDAgMSAxIDAgMTYgOCA4IDAgMCAxIDAtMTZ6bS0xIDQuNWMtMi4yIDAtMy41IDEuNi0zLjUgMy41czEuMyAzLjUgMy41IDMuNWMxLjEgMCAyLS41IDIuNi0xLjJsLTEuMi0xLjJjLS40LjUtMSAuOC0xLjYuOC0xLjIgMC0yLS45LTItMnMuOC0yIDItMmMuNiAwIDEuMS4yIDEuNS42bDEuMi0xLjJjLS43LS43LTEuNy0xLjEtMi45LTEuMXoiLz48L3N2Zz4%3D&logoColor=white" alt="License">
</p>

<p>
  <sub>42 種類の加工モード &middot; 加工パイプライン &middot; 設定ウィンドウ &middot; お気に入り &middot; 画面 OCR &middot; 暗号化履歴 &middot; 連続記録 &middot; 機密情報マスキング</sub>
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
./ClipRefiner.exe
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
| **設定ウィンドウ** | トレイから GUI で設定編集 (8 タブ)。正規表現プレビュー・パイプライン・ホットキー編集など。変更は自動保存 |
| **お気に入り変換** | よく使うモードを専用ホットキー (最大 20 件) で即切替。スロットは設定ウィンドウで変更・無効化可 |
| **画面 OCR** | 範囲選択して OS 標準 OCR でクリップボードへ (`Alt+Shift+O`) |
| **連続記録** | コピーした元テキストを FIFO キューへ蓄積 (有効中は自動加工停止。有効化時点の内容は記録しない)。`Ctrl+V` やメニューから順に取り出し (`Alt+Shift+C`) |
| **正規表現モード** | `config.toml` の `[regex]` で置換・抽出・削除・分割 |
| **履歴** | 加工結果をメモリ上のみ暗号化保持 (オプション。デフォルト無効) |
| **加工の取り消し** | 直近 1 件を元テキストへ復元 (`Alt+Shift+Z`) |
| **通知の個別設定** | 加工完了・一時停止・OCR など種類ごとに ON/OFF (マスター OFF 時もエラーは常に表示) |
| **機密マスキング** | 通知・履歴・登録クリッププレビューで API キー等を非表示 |
| **カスタム監視周期** | ポーリング間隔を 100〜60000 ms で指定 (トレイはプリセット +「カスタム…」) |

---

## 使用方法

### 監視モード

トレイアイコンの右クリックメニュー、または [設定ウィンドウ](#設定ウィンドウ) から加工モード・監視方式・履歴・連続記録などを操作する。`config.toml` の `pipeline` で複数モードを連鎖適用できる。監視方式 (`Polling` / `Event`) の違いは [CONFIG.md](CONFIG.md#監視方式-monitor_mode) を参照。連続記録の詳細は [CONFIG.md の連続記録](CONFIG.md#連続記録) を参照。

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

`--e2e-window` は開発・テスト用 (Playwright E2E)。利用者向けではない。詳細は [DEVELOPMENT.md](DEVELOPMENT.md#webview-e2e-playwright) を参照。

---

## UI と操作

### システムトレイ

変換モード・監視方式/周期・履歴・連続記録・登録クリップ・通知・**設定ウィンドウ**の開閉・一時停止・終了などを提供する。通知はトレイメニューまたは設定ウィンドウ「通知」タブから、マスター ON/OFF と種類別トグル (加工完了・一時停止・履歴復元・OCR・登録クリップ・お気に入り・連続記録、クリップボード内容の表示) を設定できる。メニュー項目の詳細は [CONFIG.md の UI 操作](CONFIG.md#ui-操作) を参照。

### 設定ウィンドウ

トレイ「設定を開く」で表示する。変更は debounce 後に `config.toml` へ自動保存され、トレイ・監視へ即時反映される。

| タブ | 主な内容 |
| :--- | :--- |
| **一般** | ログイン時の自動起動 |
| **お気に入り** | 変換モードの追加・並び替え・削除 |
| **正規表現** | パターン編集とライブプレビュー |
| **履歴・連続記録** | 有効化・件数・クリア |
| **登録クリップ** | 一覧・コピー・削除・新規登録 |
| **監視** | 一時停止・方式/周期 (100〜60000 ms)・デフォルトモード・パイプライン |
| **通知** | マスターと種類別トグル |
| **ショートカット** | グローバルホットキー (固定割り当て + お気に入りスロット) |

詳細は [CONFIG.md の設定ウィンドウ](CONFIG.md#設定ウィンドウ) を参照。

### クイックセレクタ / 登録クリップセレクタ

いずれもコマンドパレット風 UI。モード名・カテゴリ・CLI 名 (登録クリップはラベル・プレビュー) で検索し、`Enter` で決定、`Esc` で閉じる。

- **クイックセレクタ** (`Alt+Shift+S`): お気に入りが先頭に並ぶ。`Ctrl+D` でお気に入り登録、`Ctrl+Shift+↑/↓` で並び替え
- **登録クリップセレクタ** (`Alt+Shift+T`): テキスト・画像を一覧。`Ctrl+Enter` でクリップボード内容を新規登録、`Del` で削除。画像はサムネイル表示

### お気に入り変換モード

トレイ「変換モード → お気に入り」、クイックセレクタ、または設定ウィンドウ「お気に入り」タブから登録。既定では登録順に `Alt+Shift+1`〜`9` / `F1`〜`F11` が割り当てられ、押下で即切替・加工する。割り当ては設定ウィンドウ「ショートカット」タブまたは `hotkeys.favorite_mode_slots` で変更でき、空欄でスロットを無効化できる。

### 画面 OCR

`Alt+Shift+O` で全画面オーバーレイを表示し、ドラッグで範囲選択。OS 標準 OCR で認識したテキストをクリップボードへ書き込む。詳細は [CONFIG.md の画面 OCR](CONFIG.md#画面-ocr) を参照。

---

## グローバルホットキー (既定)

設定ウィンドウの「ショートカット」タブ、または `config.toml` の `[hotkeys]` で変更できる。設定ウィンドウからの変更は保存時に OS へ再登録される。`config.toml` を外部編集した場合は、監視ループが約 2 秒ごとに変更を検知して自動反映する ([CONFIG.md の外部編集](CONFIG.md#外部編集の自動再読み込み) を参照)。不正な値はデフォルトへ補正される。

| ホットキー | 動作 |
| :--------- | :--- |
| `Alt+Shift+S` | クイックセレクタ |
| `Alt+Shift+T` | 登録クリップセレクタ |
| `Alt+Shift+O` | 画面 OCR |
| `Alt+Shift+P` | 監視の一時停止 / 再開 |
| `Alt+Shift+C` | 連続記録の有効 / 無効 |
| `Alt+Shift+Z` | 加工の取り消し |
| `Alt+Shift+N` | デスクトップ通知のマスター ON/OFF (エラー通知は常に表示) |
| `Alt+Shift+Q` | 終了 |
| `Alt+Shift+1`〜`9`, `F1`〜`F11` | お気に入り変換 (登録順。スロットごとに変更・無効化可) |

形式の詳細は [CONFIG.md のホットキー形式](CONFIG.md#ホットキー形式) を参照。

---

## 履歴・連続記録・セキュリティ

**履歴**はトレイ「履歴」から有効化できる (デフォルト無効)。加工結果を最大 `history_limit` 件 (既定 10) 保持し、トレイから呼び出せる。

**連続記録**は履歴とは別の FIFO キュー。トレイ「連続記録」または `Alt+Shift+C` で有効化し、監視時にコピーした元テキストを加工せず蓄積する (有効化時点のクリップボード内容は記録せず、以降のコピーから積む)。有効中はコピー時の自動加工が停止し、トレイの変換モードメニューは無効化される。連続記録 ON 時の `Ctrl+V` でキュー先頭をクリップボードへ載せてから貼り付ける。詳細は [CONFIG.md の連続記録](CONFIG.md#連続記録) を参照。

履歴・連続記録・加工取り消し用テキストは**ディスクへ書かず**、メモリ上で ChaCha20-Poly1305 暗号化して保持する。登録クリップのみ、ChaCha20-Poly1305 で暗号化して設定ディレクトリへ永続化する (鍵は Windows DPAPI で保護)。通知や履歴ラベルでは機密らしい内容を自動マスクする。多層的な保護の詳細と利用上の注意は [CONFIG.md のセキュリティ](CONFIG.md#クリップボード履歴のセキュリティ) を参照。

---

## インストール

リリース済みパッケージがある場合 (対応 OS は **Windows のみ**):

| プラットフォーム | 方法 |
| :--------------- | :--- |
| **Windows** | MSI を実行 (per-user、管理者権限不要。`THIRD_PARTY_NOTICES` を同梱) |

ソースからのビルド・パッケージ作成は [DEVELOPMENT.md](DEVELOPMENT.md) を参照。実行ファイル単体のポータブル利用も可能 (設定はユーザの設定ディレクトリへ保存)。

**ログイン時の自動起動:** 設定ウィンドウ「一般」タブから切替 (レジストリ Run キー)。実行ファイルを移動した場合は一度オフにしてから再度オンにする。

---

## 設定

| プラットフォーム | パス |
| :--------------- | :--- |
| **Windows** | `%APPDATA%\ClipRefiner\config.toml` |

日常的な変更は [設定ウィンドウ](#設定ウィンドウ) から行うのが簡単。設定項目・加工パイプライン・監視方式・処理上限・ログの場所・加工モード一覧・使用例は **[CONFIG.md](CONFIG.md)** を参照。ビルド・テスト・ライブラリ利用は **[DEVELOPMENT.md](DEVELOPMENT.md)** を参照。

---

<div align="center">

## ライセンス

[All Rights Reserved](LICENSE)

</div>
