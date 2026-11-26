# リリース

リリースの準備から公開までの手順をまとめる。バージョンごとの利用者向け変更内容は [CHANGELOG.md](../../CHANGELOG.md) に記載する ([Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) 形式)。

手順の全体は次のとおり。

1. [準備](#準備): ツールと鍵をそろえ、CHANGELOG とドキュメントを仕上げる
2. [リリーススクリプト](#リリーススクリプト-releaseps1): `release/v{version}` ブランチで `release.ps1` を実行し、バージョン更新・品質チェック・コミット・MSI / ポータブル ZIP の作成と署名を一括で行う
3. [リリース後の作業](#リリース後の作業): ブランチの反映・タグ付け・アセットの公開と配信の確認

## 準備

### 環境

- MSI / ポータブル ZIP を作る場合は `minisign` をインストールし、環境変数 `CLIP_REFINER_SIGNING_KEY` に秘密鍵のパスを設定しておく ([更新署名鍵](#更新署名鍵)。`-DryRun` でも検証する)
- MSI / ポータブル作成には [Windows パッケージ作成](setup.md#windows-パッケージ作成) と同じ前提 (`cargo-wix`、`cargo-about`、mdBook、WiX Toolset v3。ポータブルのみなら `cargo-about` と mdBook で可)

### 変更内容の仕上げ

1. [CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` 節へ変更内容を記載しておく (空だと `release.ps1` が失敗する)
2. 設定スキーマ (`consts::CONFIG_VERSION`) を変更したリリースでは、CHANGELOG の **Settings** 節に移行内容を書く
3. 利用者向けの大きな変更があれば [README.md](../../README.md) / `docs/user/` の該当文書も合わせて更新する。設定スキーマ番号・既定値・上限のずれは `crates/clip-refiner-core/src/config/spec.rs` のテストが検出する ([ドキュメント同期](contributing.md#ドキュメント同期))
4. トレイ実クリック・実ホットキー競合・トースト見た目など、[手動確認が残る範囲](testing.md#手動確認が残る範囲) に該当する変更だけ実機確認する

## リリーススクリプト (`release.ps1`)

`scripts/windows/release.ps1` で、バージョン更新から MSI / ポータブル ZIP 作成までを一括実行できる。`release/v{version}` ブランチ上で実行し (例: `release/v1.1.0` で `-Version 1.1.0`)、既定では作業ツリーがクリーンであること (`-AllowDirty` で上書き可) を求める。

```powershell
# 最新の origin/develop からリリースブランチを作成して checkout (バージョンとブランチ名が一致必須)
git fetch origin develop
git switch -c release/v1.1.0 origin/develop

# 変更内容の確認のみ (ファイル・Git・MSI・ポータブル ZIP は触らない)
./scripts/windows/release.ps1 -Version 1.1.0 -DryRun

# 明示的なバージョン指定
./scripts/windows/release.ps1 -Version 1.1.0

# パッチリリース (1.0.0 -> 1.0.1)。事前に release/v1.0.1 ブランチへ checkout すること
./scripts/windows/release.ps1 -Bump Patch
```

**自動で行うこと:**

1. 現在のブランチが `release/v{version}` であることを検証 (不一致なら失敗)
2. ルート `Cargo.toml` の `[workspace.package] version` を更新 (`clip-refiner-core` は `workspace.dependencies` の path 参照のため版の二重更新は不要)
3. `Cargo.lock` の workspace パッケージ version を同期
4. [CHANGELOG.md](../../CHANGELOG.md) の `[Unreleased]` 節を `## [x.y.z] - YYYY-MM-DD` へ移し、空の `[Unreleased]` と compare リンクを更新
5. `check.ps1` で [品質チェック](testing.md#品質チェック) (E2E を含む全項目)。pre-push は E2E を含まないため、E2E はここで必ず通す (`CLIP_REFINER_SKIP_E2E` は無効化する)
6. `Release: v{version}` で Git コミット (`Cargo.lock` を含む)
7. `build-msi.ps1` で MSI を作成し内容を検証 (`THIRD_PARTY_NOTICES`・利用者向けドキュメント同梱。mdBook が必要)
8. `build-portable.ps1` でポータブル ZIP を作成する (MSI 作成済みなら `-SkipBuild` / `-SkipNotices` で成果物を再利用)
9. 作成した MSI / ポータブル ZIP ごとに `minisign` で `<asset>.minisig` を作り、`keys/update-signing/*.pub` で検証する (秘密鍵がパスワード保護なら各アセットで入力を求められる)。署名の準備 (`minisign`・`CLIP_REFINER_SIGNING_KEY`・公開鍵) はビルド前に確認する

**主なオプション:**

| オプション | 説明 |
| :--- | :--- |
| `-Bump Patch\|Minor\|Major` | 現在のバージョンから SemVer インクリメント |
| `-Version x.y.z` | 新バージョンを明示指定 (`-Bump` と排他) |
| `-Date yyyy-MM-dd` | リリース日 (既定: 本日) |
| `-DryRun` | 更新予定を表示するのみ |
| `-SkipCheck` | 品質チェックをスキップ |
| `-SkipMsi` | MSI 作成をスキップ |
| `-SkipPortable` | ポータブル ZIP 作成をスキップ |
| `-NoCommit` | Git コミットをスキップ |
| `-AllowDirty` | 未コミット変更があっても続行 |
| `-Target Host\|x64` | 配布 CPU (省略時はホスト) |

## リリース後の作業

1. `git push -u origin release/v{version}` でリモートへ反映する
2. `release/v{version}` から `main` と `develop` へ、それぞれ Pull Request を作ってマージする。`main` と競合する場合は、先に `main` を release ブランチへマージしてから Pull Request を作る
3. Gitea 上で `v{version}` タグを付与する
4. GitHub Release へ MSI (`target/wix/`) / ポータブル ZIP (`target/portable/`) と、それぞれの署名ファイル (`*.msi.minisig` / `*-portable.zip.minisig`) をアップロードする
5. GitHub Release を公開し (ドラフト・プレリリースはアプリの更新対象にならない)、次を確認する。アプリは digest と署名のどちらかが欠けたアセットを適用しない ([アプリ更新の検証](../user/security.md#アプリ更新の検証))
   - `https://api.github.com/repos/GnudMna/clip-refiner/releases/latest` がこの版を返し、各アセットに `digest` (`sha256:<hex>`) がある
   - 署名ファイルをアセットと同じ URL 形式 (`https://github.com/GnudMna/clip-refiner/releases/download/{tag}/{asset}.minisig`) で取得できる
   - 4.0.1 以前のアプリは [gh-release-bridge](https://gh-release-bridge.gnudlemna-4656.workers.dev) からしか更新できない。bridge を止めるまでは、bridge の Worker Secret `CLIP_REFINER_KEY` を変えず (配布済みのアプリに同じ値が埋め込まれている)、その値を `Authorization: Bearer` で送って、この版のアセットと署名ファイル (`/{owner}/{repo}/{tag}/{asset}` と `.minisig`) を取得できる

## 更新署名鍵

アプリ更新のアセットは minisign (Ed25519) で署名する。公開鍵は `keys/update-signing/*.pub` に置き、`build.rs` がビルド時にすべて埋め込む。秘密鍵はリポジトリ・CI に置かない。

```powershell
# minisign の導入
winget install jedisct1.minisign

# 鍵ペアの生成 (秘密鍵はリポジトリ外。パスワード保護を推奨)
minisign -G -p keys/update-signing/release-2026.pub -s "$env:USERPROFILE\.minisign\clip-refiner-release.key"

# release.ps1 で使う秘密鍵のパス
$env:CLIP_REFINER_SIGNING_KEY = "$env:USERPROFILE\.minisign\clip-refiner-release.key"
```

- 公開鍵を追加・変更した版を配布してから、その鍵で署名した更新を出す。利用者の手元のアプリは自分に埋め込まれた公開鍵でしか検証できない
- 鍵の入れ替えは、新旧の公開鍵を両方置いた版を一度配布し、以降の版で旧鍵を外す。秘密鍵を紛失した場合や漏洩した場合も同じ手順で、旧鍵を外した版は手動で配布する
- 公開鍵が 1 つも無いビルドは更新を適用しない (開発ビルドでの誤適用を防ぐ)。`release.ps1` は公開鍵が無いと失敗する
