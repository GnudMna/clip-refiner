# 変更履歴

このファイルは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) の形式に従う。

バージョン番号は [Semantic Versioning](https://semver.org/lang/ja/) に準拠する。各リリースの **設定** 節は `config.toml` の `version` (設定スキーマ) に関する変更を示す。

## [Unreleased]

### Added

- [CONFIG.md](CONFIG.md) を追加し、設定リファレンスを README から分離
- [DEVELOPMENT.md](DEVELOPMENT.md) にスレッドモデル (Event loop / クリップボードワーカー / ホットキー) の説明を追記
- 画面 OCR を macOS (Apple Vision) / Linux (Tesseract) で利用可能に (`Alt+Shift+O`、WebView オーバーレイ)
- Linux 向け画面 OCR 用の `ocr_overlay.html` と `xcap` による領域キャプチャ
- ライブラリ API: `config` モジュールの公開、加工パイプライン API (`apply_text_pipeline`, `apply_pipeline_to_text`, `split_pipeline`)、クレートルート re-export、`RefineContext::with_regex`

### Changed

- `tray/hotkey` を解決・登録・イベント処理のサブモジュールへ分割
- `platform/ocr_overlay/windows` を型・座標変換・描画・ウィンドウプロシージャのサブモジュールへ分割
- 全 OS で有効になった `RefineMode::is_supported_on_current_platform` と `normalize_platform_modes` を除去
- README を概要・クイックスタート中心に簡素化し、加工モード一覧・UI 操作・使用例・セキュリティ詳細を [CONFIG.md](CONFIG.md) へ移行
- [CONFIG.md](CONFIG.md) に加工モード一覧・UI 操作・画面 OCR・履歴セキュリティ・使用例・ログイン時自動起動の節を追加
- 登録クリップのドキュメント記述を `registered-clips.dat` 暗号化保存 (設定 v2) に合わせて修正
- クリップボードワーカー初期化失敗時に UI へ通知し、自動復旧を試みるように変更
- 設定 `version` がアプリより新しい場合、全項目のデフォルト化ではなく `version` を現行スキーマへクランプして既存値を保持するように変更
- `excel-to-image` 加工モードを macOS / Linux でも利用可能に (Windows 専用の `CF_DIB` フォールバックは Windows のみ)
- OCR 前処理 (小画像の拡大・日本語空白除去) を `platform/ocr/normalize.rs` / `prepare.rs` へ共通化

### Fixed

- クリップボード初期化失敗後に監視だけ停止しトレイ常駐が続く状態を解消

## [0.9.0] - 2026-07-01

### Added

- 登録クリップへの画像登録と、登録クリップセレクタでのサムネイルプレビュー

### Changed

- セレクター UI の共通 JS / CSS を `selector-common.js` へ抽出
- **設定 (v1 → v2)**: `[[clips]]` を `config.toml` から分離し、暗号化ファイル `registered-clips.dat` へ保存。初回起動時に既存 `[[clips]]` を自動移行

### Security

- 登録クリップを ChaCha20-Poly1305 で暗号化してディスクへ保存 (平文の `config.toml` 保存を廃止)

## [0.8.0] - 2026-06-28

### Added

- お気に入り変換モード (最大 20 件、専用ホットキー `Alt+Shift+1`〜`9` / `F1`〜`F11`)
- 加工パイプライン (`pipeline`、最大 10 段のモード連鎖)
- 画面 OCR (Windows のみ、`Alt+Shift+O` で範囲選択)
- ケース変換モード 5 種 (`to-camel-case` など)
- `excel-to-image` 加工モード (Windows のみ)

### Changed

- **設定 (v0 → v1)**: `favorite_modes` フィールドを追加
- 起動失敗時のデスクトップ通知とエラーメッセージを強化

### Fixed

- トレイ操作時のお気に入り連携とサブメニュー選択でデッドロックが起きる問題
- 非対応 OS の UI に Windows 専用加工モードが表示される問題

## [0.7.1] - 2026-06-27

### Added

- 設定ファイルの外部編集を検知してホットキー・監視設定などを再起動なしで再読み込み (トレイの「設定を再読み込み」でも即時反映)

### Changed

- 正規表現パターンのコンパイル結果をキャッシュし、繰り返し加工時のオーバーヘッドを削減

### Fixed

- Windows のデスクトップ通知が ClipRefiner 名義で表示されず、アイコンが未登録になる問題

## [0.7.0] - 2026-06-26

### Added

- 登録クリップ (よく使うテキストの保存・ホットキー / セレクタからコピー)
- 正規表現加工モード 4 種 (`regex-replace` / `regex-extract` / `regex-delete` / `regex-split`)
- `markdown-to-excel` 加工モード
- 設定スキーマのバージョン移行 (`config/migrate.rs`)
- Linux (deb) / macOS (DMG) インストーラー生成スクリプト
- 初回起動時に説明コメント付き `config.toml` を自動生成

### Changed

- **設定**: 保存形式を JSON から TOML へ変更 (`config.toml`)
- 通知にクリップボード内容を含めるかどうかを個別に設定可能に
- アプリ表示名を ClipRefiner に統一

### Security

- クリップボード上限 (2 MiB) とパーサー入力上限の適用
- 通知・履歴メニュー・登録クリッププレビューでの機密情報マスキング
- Unix / Windows で設定ディレクトリ・ログファイルのアクセス制限
- 履歴・取り消し用テキストの平文メモリ保持を削減 (`SecretString` / ゼロ化)

## [0.6.0] - 2026-06-24

### Added

- 加工の取り消し (`Alt+Shift+Z`)
- ログイン時の自動起動 (Windows / macOS / Linux)
- トレイメニュー「設定を開く」
- Linux Wayland でのイベント方式クリップボード監視 (`ext-data-control` / `wlr-data-control`)

### Changed

- Windows MSI を per-user インストールに変更 (管理者権限不要)

## [0.5.0] - 2026-06-18

### Added

- ホットキー・監視周期・通知などを `config` でカスタマイズ可能に

### Changed

- クイックセレクターの UI を改善
- 加工結果通知の表示内容を調整

### Security

- クリップボード履歴をメモリ内で ChaCha20-Poly1305 暗号化 (平文の常駐・ディスク書き込みなし)

## [0.4.1] - 2026-06-16

### Added

- macOS / Linux でイベント方式のクリップボード監視に対応

### Changed

- `AppState` の設定アクセス API を整理

### Fixed

- 一時停止中でも監視スレッドが動作し続ける問題
- 二重加工防止ロジックの副作用で意図しないスキップが起きる問題
- ホットキーでの一時停止が `config` に保存されない問題
- CSV 判定・数値判定・パニックリスクなど複数の安定性問題

## [0.4.0] - 2026-04-12

### Changed

- リリースビルドのログ出力をファイルのみに限定 (標準出力への出力はデバッグビルドのみ)
- ログローテーションのオーバーヘッドを改善
- 監視スレッドの停止・再開の挙動を改善

### Fixed

- UTM 除去で URL フラグメントが欠落する問題
- 複数行トリム時に CRLF が LF に変わる問題
- JSON の `\uXXXX` アンエスケープ未対応
- CSV 変換失敗時にクリップボードが空になる問題
- パス判定の誤検知

## [0.3.2] - 2026-03-21

### Added

- ファイルベースのロギング (日次ローテーション、`logs/` 配下)
- 一時停止状態の `config` への永続化

### Changed

- 設定・ログの保存先取得に `directories` クレートを使用
- イベントループとアプリ機能のモジュール分割

## [0.3.1] - 2026-02-16

### Added

- `path-to-slash` / `path-to-backslash` 加工モード
- 通知の種類 (モード変更 / 結果 / 一時停止) を個別に ON/OFF 可能に

### Changed

- 一時停止時にクリップボード監視スレッドを終了するように変更

## [0.3.0] - 2026-02-08

### Added

- クイックセレクター (`Alt+Shift+S`、コマンドパレット風 UI)
- グローバルホットキー (一時停止・終了・セレクター表示など)
- パス編集系加工モード (`extract-basename` など)
- `sort-lines-desc` 加工モード

### Changed

- 加工ロジックを `Refiner` トレイトへ整理
- アイコン画像を更新

## [0.2.1] - 2026-02-01

### Added

- クリップボード履歴 (トレイメニューから過去の加工結果を呼び出し)
- デスクトップ通知 (加工完了・モード変更など)
- Excel → Markdown 変換
- 行操作 (`remove-empty-lines` / `remove-duplicate-lines`)
- エスケープ / アンエスケープ、日時変換モード

### Changed

- トレイメニューをカテゴリ別サブメニューへ再構成
- 変換モードメニューに現在のモードのチェック表示を追加

## [0.2.0] - 2026-01-29

### Added

- イベント方式のクリップボード監視 (Windows / macOS。ポーリングとの切り替え可能)
- `markdown-to-html` 加工モード

## [0.1.1] - 2026-01-24

### Added

- YAML ↔ JSON 変換 (キー順序保持版を含む)
- `trim-lines` 加工モード
- キー順序保持版 JSON 整形 (`json-format-preserve-order`)
- 設定の自動保存と、読み込み失敗時の通知

## [0.1.0] - 2026-01-22

### Added

- 行ソート (`sort-lines-asc`)
- UTM パラメータ削除 (`remove-utm`)
- 数値のカンマ付与 / 除去
- JSON 整形、トリム、URL エンコード / デコード

## [0.0.3] - 2026-01-21

### Changed

- クリップボード監視ループと変換ロジックのリファクタリング
- エラーハンドリングの強化
- `Clipboard` インスタンスの使い回し

## [0.0.2] - 2026-01-20

### Changed

- トレイメニューで加工モードを切り替えた際、直ちにクリップボードへ変換を適用
- Windows 実行ファイルへバージョン・説明などのプロパティを埋め込み

## [0.0.1] - 2026-01-20

### Added

- システムトレイ常駐とクリップボード監視 (ポーリング)
- 多重起動防止
- エラー時のデスクトップ通知

[Unreleased]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.9.0...develop
[0.9.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.8.0...v0.9.0
[0.8.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.7.1...v0.8.0
[0.7.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.7.0...v0.7.1
[0.7.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.6.0...v0.7.0
[0.6.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.5.0...v0.6.0
[0.5.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.4.1...v0.5.0
[0.4.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.4.0...v0.4.1
[0.4.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.2...v0.4.0
[0.3.2]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.1...v0.3.2
[0.3.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.0...v0.3.1
[0.3.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.2.1...v0.3.0
[0.2.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.2.0...v0.2.1
[0.2.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.1.1...v0.2.0
[0.1.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.1.0...v0.1.1
[0.1.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.3...v0.1.0
[0.0.3]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.2...v0.0.3
[0.0.2]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.1...v0.0.2
[0.0.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/src/tag/v0.0.1
