# 開発環境の構築とビルド

開発環境の準備・ビルド・配布パッケージの作成をまとめる。テストと品質チェックは [testing.md](testing.md)、コードの構成は [architecture.md](architecture.md)、リリース手順は [release.md](release.md) を参照。

## 前提条件

開発・実行の対象 OS は **Windows** のみ。必要なツールは作業内容によって異なる。

| 用途 | 必要なもの |
| :--- | :--- |
| Rust のビルド・テスト | [Rustup](https://www.rust-lang.org/tools/install) (`rust-toolchain.toml` により Rust 1.96、rustfmt、Clippy を使用)、Visual Studio 2022 以降の **Desktop development with C++** ワークロード。ARM64 ネイティブまたはクロスコンパイルには **MSVC C++ ARM64/ARM64EC ビルドツール** と `rustup target add aarch64-pc-windows-msvc` も必要 (パッケージスクリプトが未導入なら追加する) |
| `scripts/windows/*.ps1` | [PowerShell 7](https://learn.microsoft.com/powershell/scripting/install/installing-powershell-on-windows) |
| UI の静的解析・JS ロジックテスト | [Node.js](https://nodejs.org/) と npm |
| WebView E2E | 対話可能なデスクトップセッション、[WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) |
| サプライチェーン監査 | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) |
| Rust カバレッジ | [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) と `rustup component add llvm-tools-preview` (`coverage.ps1` / `check.ps1 -Coverage`) |
| MSI 作成 | `cargo-wix`、`cargo-about`、WiX Toolset v3.14.1 以上 |

`cargo build` だけなら Node.js・`cargo-deny`・MSI 用ツールは不要。`check.ps1` は Node.js、`cargo-deny`、WebView2 Runtime を含む品質チェック一式の前提を要求する (カバレッジは任意)。通常の `cargo test` は Node.js が無い場合に JS ロジックテストを `note:` 付きでスキップする。

## 初回セットアップ

```powershell
git clone <repository_url>
Set-Location clip-refiner

# Rust ツールチェーンは rust-toolchain.toml から自動選択
rustup show active-toolchain

# UI の lint・整形・E2E 用依存
npm ci

# check.ps1 のサプライチェーン監査用
cargo install cargo-deny --locked

# 任意: Rust カバレッジ計測 (coverage.ps1 / check.ps1 -Coverage)
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

任意で Git フックを有効化すると、コミット時に整形、プッシュ時に品質チェック一式を実行できる。

```powershell
./scripts/windows/install-hooks.ps1
```

## ソースからのビルド

[初回セットアップ](#初回セットアップ) のあと、リポジトリルートで実行する。ビルドだけなら Node.js は不要。

```powershell
cargo build -p clip-refiner --release
```

PowerShell スクリプトでターゲットを明示する場合:

```powershell
./scripts/windows/build.ps1 -Release
./scripts/windows/build.ps1 -Release -Target arm64
./scripts/windows/build.ps1 -Release -Target All
```

ホスト向けバイナリは `target/release/ClipRefiner.exe` に生成される。`-Target arm64` / `-Target x64` を明示した場合は `target/{triple}/release/ClipRefiner.exe` になる。

x64 ホストから ARM64 をクロスコンパイルする場合は、Visual Studio に ARM64 C++ ビルドツールが入っていること。パッケージスクリプトは未導入なら `rustup target add aarch64-pc-windows-msvc` を実行し、OS ネイティブのホストアーキテクチャで `vcvarsall.bat` (例: x64 ホストなら `x64_arm64`) を呼んで MSVC 環境を読み込む。ARM64 Windows 上の x64 rustc (エミュレーション) でも、ホストは OS 側の ARM64 ツールを使う。

## Windows パッケージ作成

### Windows MSI

`cargo-wix` と WiX Toolset v3 を使って MSI を作成できる。第三者ライセンス文書の生成に `cargo-about` も必要。

```powershell
cargo install cargo-wix --locked
cargo install cargo-about --locked --features cli
winget install WiXToolset.WiXToolset

./scripts/windows/build-msi.ps1
./scripts/windows/build-msi.ps1 -Target arm64
./scripts/windows/build-msi.ps1 -Target All
```

出力先: `target/wix/clip-refiner-{version}-{target-triple}.msi` (per-user インストール、日本語 UI)。`-Target` を省略するとホスト CPU (x64 または ARM64) 向け。`All` で両方を作成する。

`build-msi.ps1` は MSI 作成後に内容検証を自動実行する。既存 MSI のみ検証する場合:

```powershell
./scripts/windows/verify-msi.ps1
./scripts/windows/verify-msi.ps1 -Target arm64
./scripts/windows/verify-msi.ps1 -MsiPath target\wix\clip-refiner-{version}-{target-triple}.msi
```

**検証内容:**

| 区分 | 項目 |
| :--- | :--- |
| メタデータ | `Cargo.toml` の `upgrade-guid` と `wix/main.wxs` の `UpgradeCode` の一致 |
| MSI Property | `ProductName`, `ProductVersion`, `UpgradeCode`, `Manufacturer`, per-user (`ALLUSERS`) |
| MSI テーブル | 必須ファイル (`ClipRefiner.exe`, `LICENSE`, `THIRD_PARTY_NOTICES`)、Feature (`Binaries`, `Environment`, `Autostart`) |
| 展開内容 | 管理インストール (`msiexec /a`) で展開し、実行ファイルサイズ・`THIRD_PARTY_NOTICES` ヘッダー・生成物サイズ一致・PE アーキテクチャ (x64 / ARM64) を確認 |

`-SkipExtract` でデータベース検証のみに省略できる。`build-msi.ps1 -SkipVerify` でビルド後検証をスキップできる。

### ポータブル ZIP

インストール不要の配布物 (実行ファイル + `portable` マーカー) を作成できる。第三者ライセンス文書の生成に `cargo-about` が必要 (`-SkipNotices` で省略可)。

```powershell
cargo install cargo-about --locked --features cli

./scripts/windows/build-portable.ps1
./scripts/windows/build-portable.ps1 -Target arm64
./scripts/windows/build-portable.ps1 -Target All
```

出力先:

| 種類 | パス |
| :--- | :--- |
| 配布フォルダ | `target/portable/clip-refiner-{version}-{target-triple}-portable/` |
| ZIP | `target/portable/clip-refiner-{version}-{target-triple}-portable.zip` |

同梱内容: `ClipRefiner.exe` / `portable` / `LICENSE` / `THIRD_PARTY_NOTICES` / `PORTABLE.txt`

| オプション | 効果 |
| :--- | :--- |
| `-Target` | 配布 CPU。`Host` (既定) / `x64` / `arm64` / `All` / 完全な triple |
| `-SkipBuild` | 既存の `target/release/ClipRefiner.exe` (または `target/{triple}/release`) を使う |
| `-SkipZip` | フォルダのみ作成し ZIP を作らない |
| `-SkipNotices` | `THIRD_PARTY_NOTICES` を再生成しない (既存があれば同梱) |

ポータブルモードの挙動は [ポータブルモード](../user/configuration.md#ポータブルモード) を参照。

再実行時に既存配布フォルダ内の `ClipRefiner.exe` が起動中だと置き換えに失敗する。トレイから終了してから再実行すること。PowerShell 7 (`pwsh`) で実行すること (Windows PowerShell 5.1 では `#requires` により失敗する)。実行ファイルの探索は `$ProjectRoot\target` (または `CARGO_TARGET_DIR`) を使い、`cargo metadata` の JSON は日本語パス化け回避のため使わない。

## 第三者ライセンス表示 (`THIRD_PARTY_NOTICES`)

`LICENSE` は ClipRefiner 本体 (All Rights Reserved) の利用条件のみを示す。配布バイナリに含まれる OSS 依存の帰属・ライセンス全文は **`THIRD_PARTY_NOTICES`** にまとめる。

| 項目 | 内容 |
| :--- | :--- |
| 生成ツール | [cargo-about](https://github.com/EmbarkStudios/cargo-about) (`cargo install cargo-about --locked --features cli`) |
| 設定 | [`about.toml`](../../about.toml)、テンプレート [`scripts/templates/third-party-notices.hbs`](../../scripts/templates/third-party-notices.hbs) |
| 自クレート除外 | `about.toml` の `private.ignore` でワークスペース内クレート (`clip-refiner`, `clip-refiner-core`) を除外 (`deny.toml` と同方針) |
| 手動生成 | `scripts/common/generate-third-party-notices.ps1` |
| 出力 | リポジトリルートの `THIRD_PARTY_NOTICES` (`.gitignore` 対象。パッケージ作成時に自動生成) |

パッケージへの同梱:

| プラットフォーム | 同梱先 |
| :--------------- | :----- |
| **Windows MSI** | `%LOCALAPPDATA%\ClipRefiner\THIRD_PARTY_NOTICES` (`build-msi.ps1` が生成後に同梱) |
| **ポータブル ZIP** | 配布フォルダ直下の `THIRD_PARTY_NOTICES` (`build-portable.ps1` が生成後に同梱) |

## 環境変数

| 変数 | 用途 |
| :--- | :--- |
| `RUST_LOG` | ログレベル (例: `debug`, `clip_refiner=trace`) |
| `RUST_BACKTRACE` | `1` のときパニック記録にバックトレースを含める ([ログ](architecture.md#ログ)) |
| `CARGO_TARGET_DIR` | ビルド出力先。`build-portable.ps1` / `check.ps1` / E2E はこの配下の `ClipRefiner.exe` を探す |
| `CLIP_REFINER_CONFIG_DIR` | 設定ディレクトリの上書き (テスト / E2E / 明示指定向け。未設定時はポータブルの `data` または `%APPDATA%\ClipRefiner`) |
| `CLIP_REFINER_KEY` | **ビルド時**に埋め込む gh-release-bridge ダウンロードキー。`build.rs` が ChaCha20-Poly1305 で暗号化し XOR 難読化してからバイナリへ入れる (`option_env!` による平文埋め込みはしない)。Worker Secret `CLIP_REFINER_KEY` と同値。未設定なら無認証 GET (開発向け)。`release.ps1` では必須。クライアント共有シークレットのため漏洩前提で運用する |
| `CLIP_REFINER_SIGNING_KEY` | `release.ps1` が更新アセットへ署名するときの minisign 秘密鍵ファイルのパス。リポジトリ外に置く ([更新署名鍵](release.md#更新署名鍵)) |
| `CLIP_REFINER_REQUIRE_NODE` | `1` のとき Node.js 未インストールで JS ロジックテストを失敗させる (`check.ps1` が設定) |
| `CLIP_REFINER_SKIP_NODE_TESTS` | `1` のとき JS ロジックテストをスキップ |
| `CLIP_REFINER_SKIP_E2E` | `1` のとき Playwright E2E をスキップ (`check.ps1` / pre-push) |
| `CLIP_REFINER_COVERAGE` | `1` のとき `check.ps1` の通常テストを `cargo llvm-cov` に切り替える (`-Coverage` と同義) |
| `CLIP_REFINER_SKIP_HOOK` | `1` のとき Git フック (pre-commit / pre-push) をスキップ |
| `CLIP_REFINER_E2E_CDP_PORT` | E2E 用 WebView2 CDP ポート (`--e2e-window` 時) |
| `CLIP_REFINER_E2E_EXE` | E2E で起動する `ClipRefiner.exe` のパス |
