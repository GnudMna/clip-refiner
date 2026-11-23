<div align="center">

<img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">

<h1>ClipRefiner</h1>

<p>
  <strong>クリップボードのテキストをリアルタイムで監視し、指定した形式に自動変換する Windows 向けデスクトップツール</strong>
</p>

<p>
  <img src="https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPldpbmRvd3M8L3RpdGxlPjxwYXRoIGQ9Ik0zIDEyLjVWNi44bDgtMS4xdjYuOEgzem05LTcuMyAxMC0xLjR2OC43SDEyVjUuMnpNMyAxMy41aDh2NS43bC04LTEuMnYtNC41em05IDBoMTB2OC42bC0xMC0xLjR2LTcuMnoiLz48L3N2Zz4%3D&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/License-MIT-F59E0B?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPkNvcHlyaWdodDwvdGl0bGU%2BPHBhdGggZD0iTTEyIDJhMTAgMTAgMCAxIDAgMCAyMCAxMCAxMCAwIDAgMCAwLTIwem0wIDJhOCA4IDAgMSAxIDAgMTYgOCA4IDAgMCAxIDAtMTZ6bS0xIDQuNWMtMi4yIDAtMy41IDEuNi0zLjUgMy41czEuMyAzLjUgMy41IDMuNWMxLjEgMCAyLS41IDIuNi0xLjJsLTEuMi0xLjJjLS40LjUtMSAuOC0xLjYuOC0xLjIgMC0yLS45LTItMnMuOC0yIDItMmMuNiAwIDEuMS4yIDEuNS42bDEuMi0xLjJjLS43LS43LTEuNy0xLjEtMi45LTEuMXoiLz48L3N2Zz4%3D&logoColor=white" alt="License">
</p>

<p>
  <sub>55 種類の変換モード &middot; 加工パイプライン &middot; 設定ウィンドウ &middot; お気に入り &middot; 画面 OCR &middot; 暗号化履歴 &middot; 連続記録モード &middot; 機密情報マスキング</sub>
</p>

</div>

---

## 主な機能

| 機能 | 概要 |
| :--- | :--- |
| **監視モード** | トレイ常駐でクリップボード変更を検知し自動変換 (自動変換を有効にした場合。新規インストールでは無効) |
| **ワンショット** | CLI から 1 回だけ加工 |
| **加工パイプライン** | 最大 10 段のモード連鎖 (`pipeline` / `--pipeline`) |
| **正規表現プリセット** | 名前付きプリセットに複数ステップを登録し、上から順に置換・抽出・削除・分割 |
| **クイックセレクター** | `Alt+Shift+S` で開くコマンドパレット風 UI。モードを検索し、`Enter` で今のクリップボードだけ変換、`Ctrl+Enter` で自動変換に設定 |
| **お気に入り変換** | よく使うモードを専用ホットキー (最大 20 件) で今のクリップボードへ一回だけ適用 |
| **登録クリップ** | よく使うテキスト・画像を暗号化保存し即コピー (`Alt+Shift+T`)。日付などを差し込むテンプレートにも対応 |
| **画面 OCR** | 範囲選択して OS 標準 OCR でクリップボードへ (`Alt+Shift+G`) |
| **連続記録モード** | コピーした元テキストを FIFO キューへ蓄積し、`Alt+Shift+X` で先頭から順に貼り付け |
| **履歴** | 加工結果をメモリ上のみ暗号化保持 (既定は無効) |
| **加工の取り消し** | 直近 1 件を元テキストへ復元 (`Alt+Shift+Z`) |
| **機密マスキング** | 通知・履歴・登録クリッププレビューの**表示だけ** API キー等を非表示 (クリップボード本体は書き換えない) |
| **設定ウィンドウ** | 8 タブの GUI で設定を編集。正規表現ライブプレビュー・ホットキー編集など。変更は自動保存 |
| **通知の個別設定** | 加工完了・自動変換の切替・OCR など種類ごとに ON/OFF (エラー通知は常に表示) |
| **アプリ更新** | 最新版を確認し、署名とハッシュを検証してから MSI / ポータブル ZIP を適用 |

クリップボードの内容をどう扱うか (監視除外・暗号化・表示マスキング) は [セキュリティ](docs/user/security.md#履歴連続記録モードの保護) を参照。

---

## 動作要件とインストール

- **対応 OS:** Windows (x64)
- **WebView:** 設定ウィンドウとセレクターの表示に [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) が必要 (通常は Windows に導入済み)。未導入でも監視・ホットキー・ワンショットは動作する

| 形態 | 入手・配置 |
| :--- | :--- |
| **MSI** | ユーザー単位で `%LOCALAPPDATA%\ClipRefiner` へインストール。スタートメニューに **ClipRefiner** と **ClipRefiner ドキュメント** (同梱の利用者向けドキュメント) が追加される。管理者権限は不要。インストーラーではユーザー `PATH` への追加とログイン時の自動起動も選択できる |
| **ポータブル ZIP** | 展開して `ClipRefiner.exe` を直接起動。同梱の `portable` マーカーにより設定・ログは実行ファイル隣の `data\` へ保存される ([ポータブルモード](docs/user/configuration.md#ポータブルモード))。利用者向けドキュメントは `docs\index.html` |
| **実行ファイル単体** | 既定では設定・ログは [`%APPDATA%\ClipRefiner`](#設定) に保存される。同じフォルダに空の `portable` ファイルを置くとポータブル動作になる |

ソースからビルドする場合は [開発環境とビルド](docs/dev/setup.md#ソースからのビルド) を参照。

---

## クイックスタート

1. スタートメニューから起動するか、`ClipRefiner.exe` を直接起動する。ウィンドウは表示されず、通知領域 (システムトレイ) に常駐する
2. テキストをコピーし、`Alt+Shift+S` でクイックセレクターを開く。モードを選んで `Enter` を押すと、今コピーしているテキストだけを変換する。初期設定ではコピーしただけでは変換しない
3. コピーのたびに同じ変換をかけたいときは、トレイアイコンを右クリックして **変換モード** からモードを選ぶか、クイックセレクターで `Ctrl+Enter` を押す。自動変換が有効になり、以後のコピーを自動で変換する。止めるには **変換モード → 自動変換を有効にする** のチェックを外す。現在の状態はトレイアイコンのツールチップに表示される
4. 細かい設定はトレイの **設定を開く** から行う

**監視モード (常駐):** 引数なしで起動するとトレイに常駐し、自動変換が有効ならクリップボードの変更を自動で変換する。

```powershell
.\ClipRefiner.exe
```

**ワンショット:** `--mode` (`-m`) で 1 回だけ加工して終了する。複数段は `--pipeline` を使う。

```powershell
.\ClipRefiner.exe -m url-decode
.\ClipRefiner.exe --pipeline url-decode trim
```

変換モード名は CLI では kebab-case (`url-decode`)、`config.toml` では PascalCase (`UrlDecode`)。一覧は [変換モード一覧](docs/user/modes.md#変換モード一覧) を参照。

ワンショットの詳細・コマンドラインオプションとトレイ・各 UI の操作は [使い方](docs/user/usage.md) を参照。

---

## グローバルホットキー (既定)

設定ウィンドウの「ショートカット」タブ、または `config.toml` の `[hotkeys]` で変更できる。設定ウィンドウからの変更は保存時に OS へ再登録され、`config.toml` の外部編集も自動で反映される ([外部編集](docs/user/configuration.md#外部編集の自動再読み込み))。不正・重複は既定値へ補正し、設定ウィンドウから保存した場合はどの割り当てを直したかを表示する。

| ホットキー | 動作 |
| :--- | :--- |
| `Alt+Shift+S` | クイックセレクター |
| `Alt+Shift+T` | 登録クリップセレクター |
| `Alt+Shift+G` | 画面 OCR |
| `Alt+Shift+P` | 自動変換の有効 / 無効 |
| `Alt+Shift+C` | 連続記録モードの有効 / 無効 |
| `Alt+Shift+X` | 連続記録モードのキュー先頭の貼り付け |
| `Alt+Shift+Backspace` | 連続記録モードの直近 1 件取り消し |
| `Alt+Shift+Z` | 加工の取り消し |
| `Alt+Shift+N` | デスクトップ通知のマスター ON/OFF (エラー通知は常に表示) |
| `Alt+Shift+Q` | 終了 |
| `Alt+Shift+1`〜`Alt+Shift+9`, `Alt+Shift+F1`〜`Alt+Shift+F11` | お気に入り変換の一回適用 (登録順。スロットごとに変更・無効化可) |

形式の詳細は [ホットキー形式](docs/user/configuration.md#ホットキー形式) を参照。

---

## 設定

| 利用形態 | パス |
| :--- | :--- |
| **通常** (既定) | `%APPDATA%\ClipRefiner\config.toml` |
| **ポータブル** (`portable` マーカーあり) | `<実行ファイルのフォルダ>\data\config.toml` |

日常的な変更はトレイ「設定を開く」の [設定ウィンドウ](docs/user/usage.md#設定ウィンドウ) から行うのが簡単。設定項目・既定値・処理上限は [設定リファレンス](docs/user/configuration.md)、動作しないときは [トラブルシューティング](docs/user/troubleshooting.md) を参照。

---

## ドキュメント

### 利用者向け

| ドキュメント | 内容 |
| :--- | :--- |
| [使い方](docs/user/usage.md) | 監視モード・ワンショット・トレイ・設定ウィンドウ・セレクター・履歴・連続記録モード・OCR・更新・自動起動 |
| [設定リファレンス](docs/user/configuration.md) | `config.toml` の場所・設定項目と既定値・処理の制限・ホットキー形式・通知・パイプライン・ログ |
| [変換モード](docs/user/modes.md) | 全 55 モードの一覧と使用例 |
| [セキュリティ](docs/user/security.md) | 信頼境界・履歴の保護・監視除外・表示マスキング・登録クリップ・ワンショット IPC・更新の検証 |
| [トラブルシューティング](docs/user/troubleshooting.md) | 症状別の確認事項 |
| [CHANGELOG.md](CHANGELOG.md) | バージョンごとの変更履歴 |

### 開発者向け

| ドキュメント | 内容 |
| :--- | :--- |
| [開発環境とビルド](docs/dev/setup.md) | 前提条件・初回セットアップ・ビルド・MSI / ポータブル ZIP 作成 (第三者ライセンス表示・ドキュメント同梱)・環境変数 |
| [テストと品質チェック](docs/dev/testing.md) | テストの実行と構成・テスト用ヘルパー・E2E・プロパティテスト・ファジング・ベンチマーク・`check.ps1`・Git フック・カバレッジ・リンター・`cargo-deny` |
| [アーキテクチャ](docs/dev/architecture.md) | プロジェクト構成・モジュールの責務 (WebView UI・トレイ・更新・通知・スクリプト)・スレッドモデル・ログ |
| [リリース](docs/dev/release.md) | 準備・`release.ps1`・リリース後の作業・更新署名鍵 |
| [開発への参加](docs/dev/contributing.md) | Git 運用・コーディング規約・ドキュメント同期 |
| [clip-refiner-core](crates/clip-refiner-core/README.md) | 加工ロジックをライブラリとして使う |

---

<div align="center">

## ライセンス

[MIT License](LICENSE)

</div>
