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
  <sub>42 種類の変換モード &middot; 加工パイプライン &middot; 設定ウィンドウ &middot; お気に入り &middot; 画面 OCR &middot; 暗号化履歴 &middot; 連続記録 &middot; 機密情報マスキング</sub>
</p>

</div>

---

## ドキュメント

| ドキュメント                     | 内容                                                               |
| :------------------------------- | :----------------------------------------------------------------- |
| **README.md** (このファイル)     | 概要・クイックスタート・基本操作                                   |
| [CONFIG.md](CONFIG.md)           | 設定リファレンス・変換モード一覧・ホットキー・使用例・セキュリティ |
| [CHANGELOG.md](CHANGELOG.md)     | バージョンごとの変更履歴                                           |
| [DEVELOPMENT.md](DEVELOPMENT.md) | ビルド・テスト・開発者向け情報                                     |

**目次:** [動作要件](#動作要件とインストール) · [クイックスタート](#クイックスタート) · [主な機能](#主な機能) · [使用方法](#使用方法) · [UI と操作](#ui-と操作) · [アプリの更新](#アプリの更新) · [ホットキー](#グローバルホットキー-既定) · [履歴・セキュリティ](#履歴連続記録セキュリティ) · [設定](#設定) · [トラブルシューティング](#トラブルシューティング)

---

## 動作要件とインストール

- **対応 OS:** Windows
- **WebView:** 設定ウィンドウとセレクターの表示に [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) が必要 (通常は Windows に導入済み)

MSI パッケージを実行すると、`%LOCALAPPDATA%\ClipRefiner` へユーザー単位でインストールされ、スタートメニューに **ClipRefiner** が追加される。管理者権限は不要。インストーラーではユーザー `PATH` への追加とログイン時の自動起動も選択できる。

実行ファイル単体でも利用できる。既定では設定・ログは [`%APPDATA%\ClipRefiner`](#設定) に保存される。USB などへ持ち出して使う場合は、実行ファイルと同じフォルダに空の `portable` ファイルを置くと、設定・ログを実行ファイル隣の `data\` に保存する (詳細は [CONFIG.md のポータブルモード](CONFIG.md#ポータブルモード))。配布用 ZIP は `./scripts/windows/build-portable.ps1` で作成できる。

ソースからビルドする場合は [DEVELOPMENT.md](DEVELOPMENT.md#ソースからのビルド) を参照。

---

## クイックスタート

スタートメニューから起動するか、実行ファイルを直接起動する。ウィンドウは表示されず、通知領域に常駐する。

**監視モード (常駐):** 引数なしで起動するとシステムトレイに常駐し、クリップボードの変更を自動加工する。

```powershell
.\ClipRefiner.exe
```

**ワンショット:** `--mode` (`-m`) で 1 回だけ加工して終了。複数段は `--pipeline` を使う。

```powershell
.\ClipRefiner.exe -m url-decode
.\ClipRefiner.exe --pipeline url-decode trim
```

変換モード名は CLI では kebab-case (`url-decode`)、`config.toml` では PascalCase (`UrlDecode`)。[CONFIG.md の変換モード一覧](CONFIG.md#変換モード一覧) を参照。

初期設定は 1 秒間隔のポーリングと URL デコード。別の加工へ切り替えるには、通知領域の ClipRefiner アイコンを右クリックして **変換モード**を選ぶか、`Alt+Shift+S` でクイックセレクターを開く。

---

## 主な機能

| 機能                   | 概要                                                                                                                                                                                                                                            |
| :--------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **監視モード**         | トレイ常駐でクリップボード変更を検知し自動加工                                                                                                                                                                                                  |
| **ワンショット**       | CLI から 1 回だけ加工                                                                                                                                                                                                                           |
| **加工パイプライン**   | 最大 10 段のモード連鎖 (`pipeline` / `--pipeline`)                                                                                                                                                                                              |
| **クイックセレクター** | コマンドパレット風 UI でモード検索・切替 (`Alt+Shift+S`)                                                                                                                                                                                        |
| **登録クリップ**       | よく使うテキスト・画像を暗号化保存し即コピー (`Alt+Shift+T`)                                                                                                                                                                                    |
| **設定ウィンドウ**     | トレイから GUI で設定編集 (8 タブ)。正規表現プレビュー・パイプライン・ホットキー編集など。変更は自動保存                                                                                                                                        |
| **アプリ更新**         | トレイまたは設定「一般」から最新版を確認。MSI はインストーラー起動、ポータブルは ZIP 展開後に実行フォルダへ自動差し替えして再起動                                                                                                                   |
| **お気に入り変換**     | よく使うモードを専用ホットキー (最大 20 件) で即切替。スロットは設定ウィンドウで変更・無効化可                                                                                                                                                  |
| **画面 OCR**           | 範囲選択して OS 標準 OCR でクリップボードへ (`Alt+Shift+G`)                                                                                                                                                                                     |
| **連続記録**           | `Alt+Shift+C` で有効化し、コピーした元テキストを FIFO キューへ蓄積 (有効中は自動加工停止。有効化時点の内容は記録しない)。`Alt+Shift+X` で先頭から貼り付け、メニューでは任意項目を取り出す。誤コピーは `Alt+Shift+Backspace` で直近 1 件取り消し |
| **正規表現モード**     | `config.toml` の `[regex]` で置換・抽出・削除・分割                                                                                                                                                                                             |
| **履歴**               | 加工結果をメモリ上のみ暗号化保持 (オプション。デフォルト無効)                                                                                                                                                                                   |
| **加工の取り消し**     | 直近 1 件を元テキストへ復元 (`Alt+Shift+Z`)                                                                                                                                                                                                     |
| **通知の個別設定**     | 加工完了・一時停止・OCR など種類ごとに ON/OFF (マスター OFF 時もエラーは常に表示)                                                                                                                                                               |
| **機密マスキング**     | 通知・履歴・登録クリッププレビューの**表示だけ** API キー等を非表示 (クリップボード本体は書き換えない)                                                                                                                                           |
| **カスタム監視周期**   | ポーリング間隔を 100〜60000 ms で指定 (トレイはプリセット +「カスタム…」)                                                                                                                                                                       |

---

## 使用方法

### 監視モード

トレイアイコンの右クリックメニュー、または [設定ウィンドウ](#設定ウィンドウ) から変換モード・監視方式・履歴・連続記録などを操作する。`config.toml` の `pipeline` で複数モードを連鎖適用できる。監視方式 (`Polling` / `Event`) の違いは [CONFIG.md](CONFIG.md#監視方式-monitor_mode) を参照。連続記録の詳細は [CONFIG.md の連続記録](CONFIG.md#連続記録) を参照。

### ワンショットモード

```powershell
.\ClipRefiner.exe -m json-format
.\ClipRefiner.exe -m regex-replace --regex-pattern "(\d{4})-(\d{2})-(\d{2})" --regex-replacement "$1/$2/$3"
```

正規表現オプションはワンショット時のみ有効。未指定の項目は `config.toml` の `[regex]` を使用する。常駐時の監視加工も `[regex]` を使用する。

トレイ常駐中のワンショットは名前付きパイプ IPC で常駐へ委譲し、監視による再加工を避ける。常駐が無い / 接続できない場合はローカル実行にフォールバックする。パイプは現在ユーザー専用 DACL とリモート拒否で保護する (詳細は [CONFIG.md のワンショット IPC](CONFIG.md#ワンショット-ipc))。常駐プロセスは多重起動を防止するが、ワンショットは常駐中も実行できる。

### コマンドラインオプション

| オプション                                      | 説明                                     |
| :---------------------------------------------- | :--------------------------------------- |
| `-m`, `--mode <MODE>`                           | ワンショットで実行する変換モード         |
| `--pipeline <MODE>...`                          | 順に適用するモード列 (`--mode` より優先) |
| `--regex-pattern`, `--regex-replacement`        | 正規表現の上書き (ワンショット時)        |
| `--regex-case-insensitive`, `--regex-multiline` | 正規表現フラグ                           |
| `-h`, `--help` / `-V`, `--version`              | ヘルプ / バージョン                      |

`--e2e-window` は開発・テスト用 (Playwright E2E)。利用者向けではない。詳細は [DEVELOPMENT.md](DEVELOPMENT.md#webview-e2e-playwright) を参照。

---

## UI と操作

### システムトレイ

変換モード・監視方式/周期・履歴・連続記録・登録クリップ・通知・**更新を確認**・**設定ウィンドウ**の開閉・一時停止・終了などを提供する。通知はトレイメニューまたは設定ウィンドウ「通知」タブから、マスター ON/OFF と種類別トグル (加工完了・一時停止・履歴復元・OCR・登録クリップ・お気に入り・連続記録、クリップボード内容の表示) を設定できる。メニュー項目の詳細は [CONFIG.md の UI 操作](CONFIG.md#ui-操作) を参照。

### アプリの更新

トレイ「更新を確認…」、または設定ウィンドウ「一般」タブの「更新を確認」から最新リリースを調べる。配信は [gh-release-bridge](https://gh-release-bridge.gnudlemna-4656.workers.dev/GnudMna/clip-refiner) 経由で、GitHub プライベート Release の MSI / ポータブル ZIP のみを対象とする。

- **インストール版 (MSI):** 新しい版があれば「ダウンロードして適用」で MSI を取得し `msiexec` を起動したあとアプリを終了する
- **ポータブル:** ZIP を一時フォルダへ展開し、差し替えヘルパーがアプリ終了を待って実行フォルダへ上書きコピーしたあと新しい版を起動する (`data\` は保持)。ヘルパー起動に失敗した場合のみ展開先をエクスプローラーで開き、手動差し替えを案内する
- **完全性検証:** version API の各アセットに付与された `digest` (`sha256:<hex>`、GitHub Release Asset の digest) を必須とし、ダウンロード後にサイズと SHA-256 が一致した場合のみ適用する。digest が無いリリースは更新対象にしない

起動時の自動チェックは行わない。設定項目以外の詳細・トラブル時の確認事項は [CONFIG.md のアプリの更新](CONFIG.md#アプリの更新) を参照。

### 設定ウィンドウ

トレイ「設定を開く」で表示する。変更は debounce 後に `config.toml` へ自動保存され、トレイ・監視へ即時反映される。

| タブ               | 主な内容                                                            |
| :----------------- | :------------------------------------------------------------------ |
| **一般**           | ログイン時の自動起動・バージョン表示・更新確認 / 適用               |
| **お気に入り**     | 変換モードの追加・並び替え・削除                                    |
| **正規表現**       | パターン編集とライブプレビュー                                      |
| **履歴・連続記録** | 有効化・件数・クリア                                                |
| **登録クリップ**   | 一覧・コピー・削除・新規登録                                        |
| **監視**           | 一時停止・方式/周期 (100〜60000 ms)・デフォルトモード・パイプライン |
| **通知**           | マスターと種類別トグル                                              |
| **ショートカット** | グローバルホットキー (固定割り当て + お気に入りスロット)            |

詳細は [CONFIG.md の設定ウィンドウ](CONFIG.md#設定ウィンドウ) を参照。

### クイックセレクター / 登録クリップセレクター

いずれもコマンドパレット風 UI。モード名・カテゴリ・CLI 名 (登録クリップはラベル・プレビュー) で検索し、`Enter` で決定、`Esc` で閉じる。

- **クイックセレクター** (`Alt+Shift+S`): お気に入りが先頭に並ぶ。`Ctrl+D` でお気に入り登録、`Ctrl+Shift+↑/↓` で並び替え
- **登録クリップセレクター** (`Alt+Shift+T`): テキスト・画像を一覧。`Ctrl+Enter` でクリップボード内容を新規登録、`Del` で削除。画像はサムネイル表示

### お気に入り変換モード

トレイ「変換モード → お気に入り」、クイックセレクター、または設定ウィンドウ「お気に入り」タブから登録。既定では登録順に `Alt+Shift+1`〜`Alt+Shift+9` / `Alt+Shift+F1`〜`Alt+Shift+F11` が割り当てられ、押下で即切替・加工する。割り当ては設定ウィンドウ「ショートカット」タブまたは `hotkeys.favorite_mode_slots` で変更でき、空欄でスロットを無効化できる。

### 画面 OCR

`Alt+Shift+G` で全画面オーバーレイを表示し、ドラッグで範囲選択。OS 標準 OCR で認識したテキストをクリップボードへ書き込む。詳細は [CONFIG.md の画面 OCR](CONFIG.md#画面-ocr) を参照。

---

## グローバルホットキー (既定)

設定ウィンドウの「ショートカット」タブ、または `config.toml` の `[hotkeys]` で変更できる。設定ウィンドウからの変更は保存時に OS へ再登録される。`config.toml` を外部編集した場合は、監視ループが約 2 秒ごとに変更を検知して自動反映する ([CONFIG.md の外部編集](CONFIG.md#外部編集の自動再読み込み) を参照)。不正な値はデフォルトへ補正される。

| ホットキー                                                    | 動作                                                     |
| :------------------------------------------------------------ | :------------------------------------------------------- |
| `Alt+Shift+S`                                                 | クイックセレクター                                       |
| `Alt+Shift+T`                                                 | 登録クリップセレクター                                   |
| `Alt+Shift+G`                                                 | 画面 OCR                                                 |
| `Alt+Shift+P`                                                 | 監視の一時停止 / 再開                                    |
| `Alt+Shift+C`                                                 | 連続記録の有効 / 無効                                    |
| `Alt+Shift+X`                                                 | 連続記録キュー先頭の貼り付け                             |
| `Alt+Shift+Backspace`                                         | 連続記録の直近 1 件取り消し                              |
| `Alt+Shift+Z`                                                 | 加工の取り消し                                           |
| `Alt+Shift+N`                                                 | デスクトップ通知のマスター ON/OFF (エラー通知は常に表示) |
| `Alt+Shift+Q`                                                 | 終了                                                     |
| `Alt+Shift+1`〜`Alt+Shift+9`, `Alt+Shift+F1`〜`Alt+Shift+F11` | お気に入り変換 (登録順。スロットごとに変更・無効化可)    |

形式の詳細は [CONFIG.md のホットキー形式](CONFIG.md#ホットキー形式) を参照。

---

## 履歴・連続記録・セキュリティ

**履歴**はトレイ「履歴」から有効化できる (デフォルト無効)。加工結果を最大 `history_limit` 件 (既定 10) 保持し、トレイから呼び出せる。

**連続記録**は履歴とは別の FIFO キュー。トレイ「連続記録」または `Alt+Shift+C` で有効化し、監視時にコピーした元テキストを加工せず蓄積する (有効化時点のクリップボード内容は記録せず、以降のコピーから積む)。有効中はコピー時の自動加工が停止し、トレイの変換モードメニューは無効化される。`Alt+Shift+X` でキュー先頭をクリップボードへ載せてから貼り付ける。誤って積んだ直近 1 件は `Alt+Shift+Backspace` またはトレイ「直近の記録を取り消す」で削除できる。詳細は [CONFIG.md の連続記録](CONFIG.md#連続記録) を参照。

履歴・連続記録・加工取り消し用テキストは**ディスクへ書かず**、メモリ上で ChaCha20-Poly1305 暗号化して保持する。登録クリップのみ、ChaCha20-Poly1305 で暗号化して設定ディレクトリへ永続化する (鍵は Windows DPAPI で保護)。通知・履歴ラベル・登録クリッププレビューでは機密らしい内容を `[機密情報のため非表示]` に自動マスクする。これは**表示置換のみ**で、クリップボード本体や履歴復元・登録クリップコピー時の本文は書き換えない。多層的な保護の詳細と利用上の注意は [CONFIG.md のセキュリティ](CONFIG.md#クリップボード履歴のセキュリティ) を参照。

---

## 設定

| 利用形態 | パス |
| :------- | :--- |
| **通常** (既定) | `%APPDATA%\ClipRefiner\config.toml` |
| **ポータブル** (`portable` マーカーあり) | `<実行ファイルのフォルダ>\data\config.toml` |

日常的な変更は [設定ウィンドウ](#設定ウィンドウ) から行うのが簡単。設定項目・加工パイプライン・監視方式・処理上限・ログの場所・変換モード一覧・使用例は **[CONFIG.md](CONFIG.md)** を参照。ビルド・テスト・ライブラリ利用は **[DEVELOPMENT.md](DEVELOPMENT.md)** を参照。

**ログイン時の自動起動:** 設定ウィンドウ「一般」タブから切替 (レジストリ Run キー)。実行ファイルを移動した場合は一度オフにしてから再度オンにする。

---

## トラブルシューティング

| 症状                         | 確認事項                                                                                                                                  |
| :--------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------- |
| 起動してもウィンドウが出ない | 常駐モードは通知領域のみで動作する。隠れているアイコンも確認する                                                                          |
| 設定・セレクターが開かない   | WebView2 Runtime 未導入の可能性。起動時・表示失敗時の案内通知から導入ページを開き、導入後にアプリを再起動する。手動なら [Microsoft Edge WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) |
| 加工されない                 | トレイで一時停止・連続記録・監視方式・選択中のモードを確認する                                                                            |
| 連続記録の貼り付けが効かない | `Ctrl+V` ではキューを排出しない。`Alt+Shift+X` (または設定した貼り付けホットキー) を使う                                                  |
| OCR ホットキーが変わった     | 既定は `Alt+Shift+G`。`config.toml` に旧値 (`Alt+Shift+O` など) が残っている場合はそちらが優先される                                      |
| ホットキーが反応しない       | 他アプリとの重複を避け、設定ウィンドウ「ショートカット」で割り当てを変更する                                                              |
| ワンショットが常駐に届かない | 常駐が起動中か確認する。届かない場合はローカル実行にフォールバックする。詳細は [CONFIG.md のワンショット IPC](CONFIG.md#ワンショット-ipc) |
| 更新確認・適用が失敗する     | ネットワークと当日ログを確認する。digest 欠落やハッシュ不一致のリリースは適用しない。[CONFIG.md のアプリの更新](CONFIG.md#アプリの更新) を参照 |
| 原因を詳しく確認したい       | 設定ディレクトリ内 `logs\` の当日ログを確認する (通常は `%APPDATA%\ClipRefiner\logs\`。ポータブル時は `data\logs\`)                      |

設定ファイルの読み込み失敗や復旧方法は [CONFIG.md の設定ファイルの場所](CONFIG.md#設定ファイルの場所)、開発環境での調査は [DEVELOPMENT.md のログ](DEVELOPMENT.md#ログ) を参照。

---

<div align="center">

## ライセンス

[All Rights Reserved](LICENSE)

</div>
