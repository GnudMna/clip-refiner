# ClipRefiner

ClipRefiner は、クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工するデスクトップツールです。
URLのデコード、UTMパラメータの削除、JSONやYAMLの整形・変換などを、コピーするだけで即座に行うことができます。

## 主な機能

- **監視モード**: システムトレイに常駐し、クリップボードの変更を検知して自動的にテキストを加工します。
- **ワンショットモード**: コマンドラインから特定のモードを指定して、現在のクリップボード内容を一度だけ加工します。
- **加工モード**:

  | モード名 (`--mode`) | 説明 |
  | :--- | :--- |
  | `url-encode` | URLエンコードを行う |
  | `url-decode` | URLデコードを行う |
  | `remove-utm` | URLからUTMパラメータ（`utm_*`）を削除する |
  | `trim` | テキスト全体の前後にある空白および改行を削除する |
  | `trim-lines` | 各行の前後にある空白を削除する |
  | `markdown-to-html` | Markdown形式をHTML形式へ変換する |
  | `json-format` | JSON形式を整形する（キー順序は不定） |
  | `json-format-preserve-order` | JSON形式を整形する（元のキー順序を保持） |
  | `json-to-yaml` | JSONをYAML形式へ変換する（キー順序は不定） |
  | `json-to-yaml-preserve-order` | JSONをYAML形式へ変換する（元のキー順序を保持） |
  | `yaml-to-json` | YAMLをJSON形式へ変換する（キー順序は不定） |
  | `yaml-to-json-preserve-order` | YAMLをJSON形式へ変換する（元のキー順序を保持） |
  | `add-comma` | 数値に3桁区切りのカンマを付与する |
  | `remove-comma` | 数値からカンマを除去する |
  | `sort-lines` | 行単位で並び替える（CSVの場合はレコード単位でソート） |

## 使用方法

### システムトレイ常駐（監視モード）

引数なしで実行すると、システムトレイ（Windows: 通知領域、macOS/Linux: ステータスバー/トレイ領域）にアイコンが表示されます。
アイコンを操作（右クリックまたはクリック）することで、加工モードの切り替え、監視の一時停止、設定の変更が可能です。

```bash
# Windows
./ClipRefiner.exe

# macOS / Linux
./clip-refiner
```

### コマンドライン実行（ワンショットモード）

特定の加工を一度だけ行いたい場合に便利です。

```bash
# クリップボード内のURLをデコード
# Windows
./ClipRefiner.exe --mode url-decode

# macOS / Linux
./clip-refiner --mode url-decode
```

## インストール・ビルド

### 動作要件

Linux環境では、GUI操作および通知機能のために以下のパッケージが必要になる場合があります（Ubuntu/Debianの例）:

```bash
sudo apt-get install libdbus-1-dev pkg-config libatk1.0-dev libgtk-3-dev
```

### ソースからビルド

Rust の開発環境が必要です。

```bash
git clone <repository_url>
cd clip-refiner
cargo build --release
```

ビルドされたバイナリは `target/release/` 内に生成されます。

## 設定

設定ファイルは以下の場所に保存されます。

- **Windows**: `%APPDATA%\ClipRefiner\config.json`
- **Linux/macOS**: `~/.config/clip-refiner/config.json`

システムトレイメニューから設定を変更すると、自動的にこのファイルに保存されます。

## ライセンス

[All Rights Reserved](LICENSE)
