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
  <sub>42 種類の加工モード &middot; 加工パイプライン &middot; お気に入り &middot; 画面 OCR (Windows) &middot; 暗号化履歴 &middot; 機密情報マスキング</sub>
</p>

<p>
  <sub>設定の詳細は <a href="CONFIG.md">CONFIG.md</a>、ビルド・開発は <a href="DEVELOPMENT.md">DEVELOPMENT.md</a> を参照</sub>
</p>

</div>

---

## 📌 目次

<table>
<tr>
<td valign="top" width="50%">

<ul>
<li><a href="#-主な機能">✨ 主な機能</a></li>
<li><a href="#️-加工モード一覧">🛠️ 加工モード一覧</a></li>
<li><a href="#-使用方法">🚀 使用方法</a>
  <ul>
  <li><a href="#監視モード-常駐">監視モード (常駐)</a></li>
  <li><a href="#ワンショットモード">ワンショットモード</a></li>
  <li><a href="#コマンドラインオプション">コマンドラインオプション</a></li>
  </ul>
</li>
<li><a href="#️-システムトレイメニュー">🖥️ システムトレイメニュー</a></li>
<li><a href="#-クイックセレクタ">🪟 クイックセレクタ</a></li>
<li><a href="#-登録クリップセレクタ">📋 登録クリップセレクタ</a></li>
<li><a href="#-お気に入り変換モード">⭐ お気に入り変換モード</a></li>
<li><a href="#-画面-ocr-windows">🔍 画面 OCR (Windows)</a></li>
<li><a href="#️-グローバルホットキー">⌨️ グローバルホットキー</a></li>
</ul>

</td>
<td valign="top" width="50%">

<ul>
<li><a href="#️-加工の取り消し">↩️ 加工の取り消し</a></li>
<li><a href="#-クリップボード履歴">🕘 クリップボード履歴</a>
  <ul>
  <li><a href="#-セキュリティ">🔒 セキュリティ</a></li>
  </ul>
</li>
<li><a href="#-ログイン時の自動起動">🚀 ログイン時の自動起動</a></li>
<li><a href="#-加工モードの使用例">📝 加工モードの使用例</a></li>
<li><a href="#-インストール">📦 インストール</a>
  <ul>
  <li><a href="#windows-msi">Windows (MSI)</a></li>
  <li><a href="#macos-dmg">macOS (DMG)</a></li>
  <li><a href="#linux-deb">Linux (deb)</a></li>
  </ul>
</li>
<li><a href="#️-設定">⚙️ 設定</a></li>
<li><a href="#-ライセンス">📄 ライセンス</a></li>
</ul>

</td>
</tr>
</table>

---

## ✨ 主な機能

<table>
<tr>
<td width="50%" valign="top">

<div style="border-left: 4px solid #4a9eff; padding: 12px 16px; margin: 8px 0; background: rgba(74, 158, 255, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔍 監視モード</strong><br>
システムトレイに常駐し、クリップボードの変更を検知して自動加工
</div>

<div style="border-left: 4px solid #7c5cff; padding: 12px 16px; margin: 8px 0; background: rgba(124, 92, 255, 0.08); border-radius: 0 8px 8px 0;">
<strong>⚡ ワンショットモード</strong><br>
CLI から特定モードを指定し、現在のクリップボードを一度だけ加工
</div>

<div style="border-left: 4px solid #00c9a7; padding: 12px 16px; margin: 8px 0; background: rgba(0, 201, 167, 0.08); border-radius: 0 8px 8px 0;">
<strong>🪟 クイックセレクタ</strong><br>
コマンドパレット風 UI で加工モードをキーボード検索・切り替え
</div>

<div style="border-left: 4px solid #ff9f43; padding: 12px 16px; margin: 8px 0; background: rgba(255, 159, 67, 0.08); border-radius: 0 8px 8px 0;">
<strong>📋 登録クリップ</strong><br>
よく使うテキストや画像を保存し、ホットキーまたはトレイから即コピー
</div>

<div style="border-left: 4px solid #a29bfe; padding: 12px 16px; margin: 8px 0; background: rgba(162, 155, 254, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔗 加工パイプライン</strong><br>
複数の加工モードを順に連鎖適用 (最大 10 段)
</div>

<div style="border-left: 4px solid #fdcb6e; padding: 12px 16px; margin: 8px 0; background: rgba(253, 203, 110, 0.08); border-radius: 0 8px 8px 0;">
<strong>⭐ お気に入り変換モード</strong><br>
よく使うモードを登録し、専用ホットキーで即切り替え
</div>

<div style="border-left: 4px solid #ff6b9d; padding: 12px 16px; margin: 8px 0; background: rgba(255, 107, 157, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔣 正規表現モード</strong><br>
<code>config.toml</code> のパターンで置換・抽出・削除・分割
</div>

</td>
<td width="50%" valign="top">

<div style="border-left: 4px solid #54a0ff; padding: 12px 16px; margin: 8px 0; background: rgba(84, 160, 255, 0.08); border-radius: 0 8px 8px 0;">
<strong>🕘 クリップボード履歴</strong><br>
加工結果をメモリ上のみで暗号化保持。平文の常駐・ディスク書き込みを避ける設計
</div>

<div style="border-left: 4px solid #feca57; padding: 12px 16px; margin: 8px 0; background: rgba(254, 202, 87, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔔 デスクトップ通知</strong><br>
加工結果・モード変更・一時停止の状態変化を通知。機密らしい内容は自動マスク
</div>

<div style="border-left: 4px solid #c44569; padding: 12px 16px; margin: 8px 0; background: rgba(196, 69, 105, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔒 機密情報マスキング</strong><br>
通知・履歴メニュー・登録クリッププレビューで API キーや JWT などを非表示
</div>

<div style="border-left: 4px solid #5f27cd; padding: 12px 16px; margin: 8px 0; background: rgba(95, 39, 205, 0.08); border-radius: 0 8px 8px 0;">
<strong>⌨️ グローバルホットキー</strong><br>
どのウィンドウからでもキー操作で機能を呼び出し
</div>

<div style="border-left: 4px solid #ee5253; padding: 12px 16px; margin: 8px 0; background: rgba(238, 82, 83, 0.08); border-radius: 0 8px 8px 0;">
<strong>↩️ 加工の取り消し</strong><br>
直近の加工をホットキーで元のテキストへ復元
</div>

<div style="border-left: 4px solid #00b894; padding: 12px 16px; margin: 8px 0; background: rgba(0, 184, 148, 0.08); border-radius: 0 8px 8px 0;">
<strong>🔍 画面 OCR (Windows)</strong><br>
画面上の範囲をドラッグ選択し、OS 標準 OCR でテキストをクリップボードへコピー
</div>

<div style="border-left: 4px solid #10ac84; padding: 12px 16px; margin: 8px 0; background: rgba(16, 172, 132, 0.08); border-radius: 0 8px 8px 0;">
<strong>🖥️ クロスプラットフォーム</strong><br>
Windows / macOS / Linux 対応。多重起動防止付き
</div>

</td>
</tr>
</table>

---

## 🛠️ 加工モード一覧

<p>
<code>--mode</code> に渡す値 (CLI 名) と、トレイメニュー・クイックセレクタでの表示は次のとおり。
<strong>全 42 モード</strong>に対応しています (<code>excel-to-image</code> は Windows のみ)。
</p>

<div style="border-left: 4px solid #7c5cff; padding: 10px 14px; margin: 12px 0; background: rgba(124, 92, 255, 0.08); border-radius: 0 6px 6px 0;">
<strong><code>config.toml</code> の <code>mode</code>:</strong> CLI は kebab-case (<code>url-decode</code> など) だが、設定ファイルでは PascalCase (<code>UrlDecode</code> など) を指定する。初回保存時に生成される <code>config.toml</code> を参照するとよい
</div>

<table>
<thead>
<tr>
<th align="left">カテゴリ</th>
<th align="left">モード名 (<code>--mode</code>)</th>
<th align="left">説明</th>
</tr>
</thead>
<tbody>
<tr>
<td rowspan="3"><strong>URL 操作</strong></td>
<td><code>url-encode</code> / <code>url-decode</code></td>
<td>URL のエンコード・デコード</td>
</tr>
<tr>
<td><code>remove-utm</code></td>
<td>URL から <code>utm_*</code> 計測パラメータを削除</td>
</tr>
<tr><td colspan="2"></td></tr>
<tr>
<td rowspan="3"><strong>パス操作</strong></td>
<td><code>extract-basename</code> / <code>extract-basename-quoted</code></td>
<td>パスからファイル名のみを抽出 (引用符付きオプションあり)</td>
</tr>
<tr>
<td><code>add-path-quotes</code> / <code>remove-path-quotes</code></td>
<td>パスへの引用符 (<code>"</code>) の付与・削除</td>
</tr>
<tr>
<td><code>path-to-slash</code> / <code>path-to-backslash</code></td>
<td>パス区切り文字をスラッシュ / バックスラッシュに変換</td>
</tr>
<tr>
<td rowspan="3"><strong>行操作</strong></td>
<td><code>sort-lines-asc</code> / <code>sort-lines-desc</code></td>
<td>行単位での昇順・降順ソート (CSV 対応)</td>
</tr>
<tr>
<td><code>remove-empty-lines</code></td>
<td>空行を削除</td>
</tr>
<tr>
<td><code>remove-duplicate-lines</code></td>
<td>重複行を削除</td>
</tr>
<tr>
<td rowspan="2"><strong>トリム</strong></td>
<td><code>trim</code></td>
<td>テキスト全体の前後の空白・改行を削除</td>
</tr>
<tr>
<td><code>trim-lines</code></td>
<td>行ごとに前後の空白を削除</td>
</tr>
<tr>
<td rowspan="2"><strong>エスケープ</strong></td>
<td><code>escape</code> / <code>unescape</code></td>
<td>バックスラッシュエスケープの付与・解除</td>
</tr>
<tr>
<td><code>regex-escape</code> / <code>regex-unescape</code></td>
<td>正規表現メタ文字のエスケープ・解除</td>
</tr>
<tr>
<td rowspan="4"><strong>正規表現</strong></td>
<td><code>regex-replace</code></td>
<td>パターンに一致する部分を置換 (<code>[regex]</code> 設定を使用)</td>
</tr>
<tr>
<td><code>regex-extract</code></td>
<td>パターンに一致する部分を行単位で抽出</td>
</tr>
<tr>
<td><code>regex-delete</code></td>
<td>パターンに一致する部分を削除</td>
</tr>
<tr>
<td><code>regex-split</code></td>
<td>パターンで分割し改行で結合</td>
</tr>
<tr>
<td rowspan="2"><strong>JSON 整形</strong></td>
<td><code>json-format</code></td>
<td>JSON をインデント整形 (キー順序不定)</td>
</tr>
<tr>
<td><code>json-format-preserve-order</code></td>
<td>JSON をインデント整形 (キー順序保持)</td>
</tr>
<tr>
<td rowspan="2"><strong>JSON へ変換</strong></td>
<td><code>yaml-to-json</code></td>
<td>YAML を JSON へ変換 (キー順序不定)</td>
</tr>
<tr>
<td><code>yaml-to-json-preserve-order</code></td>
<td>YAML を JSON へ変換 (キー順序保持)</td>
</tr>
<tr>
<td rowspan="2"><strong>YAML へ変換</strong></td>
<td><code>json-to-yaml</code></td>
<td>JSON を YAML へ変換 (キー順序不定)</td>
</tr>
<tr>
<td><code>json-to-yaml-preserve-order</code></td>
<td>JSON を YAML へ変換 (キー順序保持)</td>
</tr>
<tr>
<td><strong>Markdown</strong></td>
<td><code>markdown-to-html</code></td>
<td>Markdown を HTML へ変換</td>
</tr>
<tr>
<td rowspan="3"><strong>Excel</strong></td>
<td><code>excel-to-markdown</code></td>
<td>Excel コピーデータを Markdown テーブルへ変換</td>
</tr>
<tr>
<td><code>markdown-to-excel</code></td>
<td>Markdown 表を Excel (TSV) 形式へ変換</td>
</tr>
<tr>
<td><code>excel-to-image</code></td>
<td>Excel コピーデータの見た目を画像としてクリップボードへ保存 (<strong>Windows のみ</strong>)</td>
</tr>
<tr>
<td rowspan="2"><strong>日時変換</strong></td>
<td><code>timestamp-to-datetime</code> / <code>datetime-to-timestamp</code></td>
<td>Unix タイムスタンプ ↔ 日時文字列の変換</td>
</tr>
<tr><td colspan="2"></td></tr>
<tr>
<td rowspan="2"><strong>数値変換</strong></td>
<td><code>add-comma</code> / <code>remove-comma</code></td>
<td>数値への 3 桁カンマ区切り付与・削除</td>
</tr>
<tr><td colspan="2"></td></tr>
<tr>
<td rowspan="5"><strong>ケース変換</strong></td>
<td><code>to-camel-case</code></td>
<td>識別子を <code>camelCase</code> へ変換</td>
</tr>
<tr>
<td><code>to-snake-case</code></td>
<td>識別子を <code>snake_case</code> へ変換</td>
</tr>
<tr>
<td><code>to-pascal-case</code></td>
<td>識別子を <code>PascalCase</code> へ変換</td>
</tr>
<tr>
<td><code>to-kebab-case</code></td>
<td>識別子を <code>kebab-case</code> へ変換</td>
</tr>
<tr>
<td><code>to-screaming-snake-case</code></td>
<td>識別子を <code>SCREAMING_SNAKE_CASE</code> へ変換</td>
</tr>
</tbody>
</table>

<div style="border: 1px solid #4a9eff; border-radius: 8px; padding: 12px 16px; margin: 16px 0; background: rgba(74, 158, 255, 0.06);">
💡 <strong>ヒント:</strong> 正規表現モードは <code>config.toml</code> の <code>[regex]</code> セクションでパターン・置換文字列・オプションを設定します。各モードの入出力例は <a href="#-加工モードの使用例">加工モードの使用例</a> を参照してください。
</div>

---

## 🚀 使用方法

### 監視モード (常駐)

<p>引数なしで実行すると、システムトレイ (通知領域) にアイコンが表示され、クリップボードの監視を開始します。アイコンの右クリックメニューから加工モードの切り替えや監視の一時停止などが行えます。<code>config.toml</code> の <code>pipeline</code> を設定すると、複数モードを順に連鎖適用できます。</p>

```bash
./ClipRefiner.exe
```

### ワンショットモード

<p>特定の加工を一度だけ行いたい場合は <code>--mode</code> (短縮形 <code>-m</code>) でモードを指定します。複数モードを順に適用する場合は <code>--pipeline</code> を使います (<code>--mode</code> より優先)。常駐せずに、現在のクリップボードの内容を加工して書き戻し、すぐに終了します。</p>

```bash
# クリップボード内の URL をデコードする
./ClipRefiner.exe --mode url-decode

# 短縮形でも指定できる
./ClipRefiner.exe -m json-format

# 複数モードを順に適用する
./ClipRefiner.exe --pipeline url-decode trim

# 正規表現で置換 (config.toml の [regex] を使用)
./ClipRefiner.exe -m regex-replace

# 正規表現設定を CLI で上書き (ワンショット時のみ)
./ClipRefiner.exe -m regex-replace --regex-pattern "(\d{4})-(\d{2})-(\d{2})" --regex-replacement "$1/$2/$3"
```

### コマンドラインオプション

| オプション                                        | 説明                                                                                                               |
| :------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------- |
| <code>-m</code>, <code>--mode &lt;MODE&gt;</code> | ワンショットで実行する加工モードを指定 ([加工モード一覧](#️-加工モード一覧) 参照)                                   |
| <code>--pipeline &lt;MODE&gt;...</code>           | ワンショットで順に適用する加工モード列 (<code>--mode</code> より優先。例: <code>--pipeline url-decode trim</code>) |
| <code>--regex-pattern &lt;PATTERN&gt;</code>      | 正規表現パターン (<code>config.toml</code> の <code>regex.pattern</code> を上書き)                                 |
| <code>--regex-replacement &lt;TEXT&gt;</code>     | 置換文字列 (<code>regex.replacement</code> を上書き。<code>regex-replace</code> で使用)                            |
| <code>--regex-case-insensitive</code>             | 大文字小文字を無視 (<code>(?i)</code> 相当)                                                                        |
| <code>--regex-multiline</code>                    | 複数行モード (<code>(?m)</code> 相当)                                                                              |
| <code>-h</code>, <code>--help</code>              | ヘルプを表示                                                                                                       |
| <code>-V</code>, <code>--version</code>           | バージョンを表示                                                                                                   |

<p>正規表現オプションはワンショット実行時のみ有効です。常駐モードでは <code>config.toml</code> の <code>[regex]</code> セクションが使用されます。加工パイプラインの詳細は <a href="CONFIG.md#加工パイプライン-pipeline">CONFIG.md</a> を参照してください。</p>

---

## 🖥️ システムトレイメニュー

<p>監視モードで常駐中は、トレイアイコンの右クリックメニューから各種操作が行えます。</p>

<table>
<thead>
<tr>
<th align="left" width="22%">メニュー</th>
<th align="left">内容</th>
</tr>
</thead>
<tbody>
<tr>
<td><strong>変換モード</strong></td>
<td>加工モードをカテゴリ別のサブメニューから選択。お気に入りサブメニューから登録・解除・即切り替えも可能。<code>pipeline</code> 設定時は連鎖中の全モードにチェックが付く</td>
</tr>
<tr>
<td><strong>監視方式</strong></td>
<td><code>ポーリング</code> / <code>イベント</code> を切り替え</td>
</tr>
<tr>
<td><strong>監視周期</strong></td>
<td><code>0.5秒</code> / <code>1秒</code> / <code>2秒</code> / <code>5秒</code> から選択 (イベント方式では無効)</td>
</tr>
<tr>
<td><strong>履歴</strong></td>
<td>履歴機能の有効化・クリア、過去の加工結果の呼び出し</td>
</tr>
<tr>
<td><strong>登録クリップ</strong></td>
<td>登録済み文字列の呼び出し、クリップボードからの新規登録</td>
</tr>
<tr>
<td><strong>通知</strong></td>
<td>デスクトップ通知の有効化と、通知内容 (モード変更・クリップボードの内容・一時停止) の個別設定</td>
</tr>
<tr>
<td><strong>設定を開く</strong></td>
<td><code>config.toml</code> を既定のアプリケーションで開く</td>
</tr>
<tr>
<td><strong>設定を再読み込み</strong></td>
<td>ディスク上の <code>config.toml</code> を即時に読み直し、ホットキー・監視設定・正規表現・登録クリップなどを再起動なしで反映。履歴を無効化した場合はメモリ上の履歴もクリア。形式エラー時は通知で失敗理由を表示</td>
</tr>
<tr>
<td><strong>ショートカット一覧</strong></td>
<td>現在のグローバルホットキー割り当てを通知で表示</td>
</tr>
<tr>
<td><strong>ログイン時に起動</strong></td>
<td>OS のログイン時自動起動を有効化・無効化</td>
</tr>
<tr>
<td><strong>一時停止</strong></td>
<td>クリップボード監視の一時停止・再開</td>
</tr>
<tr>
<td><strong>終了</strong></td>
<td>アプリケーションを終了</td>
</tr>
</tbody>
</table>

---

## 🪟 クイックセレクタ

<p>コマンドパレット風のウィンドウで、加工モードをキーボードから素早く検索・選択できます。グローバルホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>S</kbd>) で表示・非表示を切り替えます。</p>

<ul>
<li><strong>検索</strong>: モード名・カテゴリ・CLI 名 (<code>--mode</code> の値) のいずれにも部分一致で絞り込み。一致部分はハイライト表示</li>
<li><strong>お気に入り</strong>: 登録済みモードは先頭の「お気に入り」セクションに表示。星印ボタンまたは <kbd>Ctrl</kbd> + <kbd>D</kbd> で登録・解除</li>
<li><strong>現在のモード</strong>: 表示時に現在選択中のモードがハイライト</li>
<li><strong>マウス操作</strong>: ホバー選択・クリック決定にも対応</li>
</ul>

| キー                                                             | 動作                                               |
| :--------------------------------------------------------------- | :------------------------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>                                      | 候補の移動                                         |
| <kbd>Home</kbd> / <kbd>End</kbd>                                 | 先頭 / 末尾へ移動                                  |
| <kbd>Enter</kbd>                                                 | 選択中のモードを決定                               |
| <kbd>Ctrl</kbd> + <kbd>D</kbd>                                   | 選択中のモードをお気に入りに登録 / 解除            |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>↑</kbd> / <kbd>↓</kbd> | お気に入り内の並び順を変更                         |
| <kbd>Esc</kbd>                                                   | 検索文字列があればクリア、空ならウィンドウを閉じる |

---

## 📋 登録クリップセレクタ

<p>よく使うテキストや画像を <code>config.toml</code> に登録し、ホットキーまたはトレイメニューからクリップボードへ即コピーできる機能です。クイックセレクタと同様のコマンドパレット風 UI (<strong>登録クリップセレクタ</strong>) も利用できます。検索文字列に一致するラベル・プレビューはハイライト表示され、画像はサムネイルプレビューが表示されます。</p>

<div style="border-left: 4px solid #ff9f43; padding: 12px 16px; margin: 12px 0; background: rgba(255, 159, 67, 0.08); border-radius: 0 8px 8px 0;">
<strong>表示:</strong> グローバルホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd>)<br>
<strong>登録:</strong> トレイメニュー「登録クリップ」→「クリップボードを登録」、またはセレクタ内で <kbd>Ctrl</kbd> + <kbd>Enter</kbd> (クリップボードに画像がある場合は画像を優先)<br>
<strong>上限:</strong> 最大 100 件、ラベル 64 文字、テキスト本文はクリップボード上限 (2 MiB) まで、画像は PNG 16 MiB / 8192 px まで
</div>

| キー                               | 動作                                               |
| :--------------------------------- | :------------------------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>        | 候補の移動                                         |
| <kbd>Home</kbd> / <kbd>End</kbd>   | 先頭 / 末尾へ移動                                  |
| <kbd>Enter</kbd>                   | 選択中の登録クリップをクリップボードへコピー       |
| <kbd>Del</kbd>                     | 選択中の登録クリップを削除                         |
| <kbd>Ctrl</kbd> + <kbd>Enter</kbd> | 現在のクリップボード内容を新規登録                 |
| <kbd>Esc</kbd>                     | 検索文字列があればクリア、空ならウィンドウを閉じる |

<p>登録クリップは <code>config.toml</code> の <code>[[clips]]</code> セクションに永続化されます。テキストのラベルは本文の先頭から自動生成され、画像は <code>registered-images/</code> ディレクトリに PNG として保存されます。</p>

```toml
[[clips]]
label = "挨拶文"
text = "お疲れ様です。よろしくお願いいたします。"

[[clips]]
label = "[画像] 800×600"
text = ""
image_file = "abc123....png"
```

---

## ⭐ お気に入り変換モード

<p>よく使う加工モードを登録し、トレイメニュー・クイックセレクタ・専用ホットキーから素早く切り替えられる機能です。</p>

<div style="border-left: 4px solid #fdcb6e; padding: 12px 16px; margin: 12px 0; background: rgba(253, 203, 110, 0.08); border-radius: 0 8px 8px 0;">
<strong>登録:</strong> トレイメニュー「変換モード」→「お気に入り」→「現在のモードをお気に入りに登録」、またはクイックセレクタで <kbd>Ctrl</kbd> + <kbd>D</kbd><br>
<strong>解除:</strong> 同サブメニューの「現在のモードをお気に入りから解除」、またはクイックセレクタで再度 <kbd>Ctrl</kbd> + <kbd>D</kbd><br>
<strong>並び替え:</strong> クイックセレクタのお気に入りセクションで <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>↑</kbd> / <kbd>↓</kbd><br>
<strong>上限:</strong> 最大 20 件 (<code>config.toml</code> の <code>favorite_modes</code> に永続化)
</div>

<p>お気に入りに登録したモードには、登録順に対応するグローバルホットキーが割り当てられます (既定: <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>1</kbd>〜<kbd>9</kbd>、<kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>F1</kbd>〜<kbd>F11</kbd>)。ホットキーを押すとそのモードへ切り替わり、現在のクリップボード内容を即座に加工します。割り当ては <code>hotkeys.favorite_mode_slots</code> でカスタマイズでき、空文字を指定したスロットは無効化されます。</p>

```toml
favorite_modes = ["UrlDecode", "Trim", "JsonFormat"]

[hotkeys]
# 1 件目: Alt+Shift+1 (省略時はデフォルト)
# 2 件目: 空文字でホットキー無効
favorite_mode_slots = ["", "Alt+Ctrl+J"]
```

---

## 🔍 画面 OCR (Windows)

<p>画面上の任意の矩形範囲をドラッグ選択し、<code>Windows.Media.Ocr</code> でテキストを認識してクリップボードへコピーする機能です。<strong>Windows のみ</strong>で利用できます。</p>

<div style="border-left: 4px solid #00b894; padding: 12px 16px; margin: 12px 0; background: rgba(0, 184, 148, 0.08); border-radius: 0 8px 8px 0;">
<strong>開始:</strong> グローバルホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd>)<br>
<strong>操作:</strong> 全画面オーバーレイ上でドラッグして範囲を選択。確定後にキャプチャと OCR を実行<br>
<strong>キャンセル:</strong> <kbd>Esc</kbd> または再度ホットキーでオーバーレイを閉じる
</div>

<ul>
<li><strong>言語パック</strong>: 日本語 OCR を優先利用。未インストール時はユーザー言語プロファイルの OCR エンジンを使用</li>
<li><strong>小さい選択範囲</strong>: 認識率向上のため、短辺が小さい画像は自動で拡大してから OCR を実行</li>
<li><strong>日本語の空白</strong>: OCR エンジンが挿入する不要なスペースを自動で除去 (英単語間のスペースは維持)</li>
<li><strong>結果</strong>: 認識テキストをクリップボードへ書き込み、デスクトップ通知で完了を知らせる</li>
</ul>

---

## ⌨️ グローバルホットキー

<p>監視モード常駐時に、アクティブなウィンドウを問わず以下のホットキーが使用できます (<code>config.toml</code> の <code>hotkeys</code> で変更可能。<code>config.toml</code> を編集した場合は自動反映、トレイの「設定を再読み込み」でも即時反映)。</p>

<table>
<thead>
<tr>
<th align="left">ホットキー</th>
<th align="left">動作</th>
</tr>
</thead>
<tbody>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>S</kbd></td>
<td>クイックセレクタの表示・非表示</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd></td>
<td>登録クリップセレクタの表示・非表示</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd></td>
<td>画面範囲選択 OCR の開始・終了 (<strong>Windows のみ</strong>)</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd></td>
<td>クリップボード監視の一時停止・再開</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Z</kbd></td>
<td>直近の加工を取り消し、加工前のテキストをクリップボードへ復元</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd></td>
<td>デスクトップ通知の ON/OFF 切り替え</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Q</kbd></td>
<td>アプリケーションの終了</td>
</tr>
<tr>
<td><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>1</kbd>〜<kbd>9</kbd><br><kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>F1</kbd>〜<kbd>F11</kbd></td>
<td>お気に入り変換モードの切り替え (登録順の 1〜20 件目。未登録スロットは無効)</td>
</tr>
</tbody>
</table>

---

## ↩️ 加工の取り消し

<p>監視モード常駐時に加工が成功した直後のみ、直近 1 件分の取り消しが可能です。ホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Z</kbd>) またはトレイメニューの「ショートカット一覧」から割り当てを確認できます。ワンショットモード (<code>--mode</code>) では利用できません。</p>

<ul>
<li><strong>対象</strong>: クリップボード監視による自動加工、またはトレイ・クイックセレクタでモードを選んで実行した手動加工のうち、直前に成功した 1 件</li>
<li><strong>動作</strong>: 加工前のテキストをクリップボードへ書き戻す</li>
<li><strong>制限</strong>: 新しい加工が成功すると取り消し対象は上書き。取り消し可能な加工がない場合は通知で知らせる (通知が有効な場合)</li>
<li><strong>セキュリティ</strong>: 加工前テキストはメモリ上のみ保持し、<code>SecretString</code> により不要になった時点でゼロ化 ([クリップボード履歴のセキュリティ](#-クリップボード履歴) 参照)</li>
</ul>

---

## 🕘 クリップボード履歴

<p>監視モードで加工したテキストを履歴として保持し、トレイメニューの「履歴」サブメニューから過去の内容をクリップボードへ呼び出せます。</p>

<ul>
<li><strong>有効化</strong>: トレイメニューの「履歴」→「履歴機能を有効にする」で切り替え (<code>config.toml</code> の <code>history_enabled</code> でも設定可能)</li>
<li><strong>保持件数</strong>: 最大 <code>history_limit</code> 件まで保持 (デフォルト 10 件、1〜100 の範囲)</li>
<li><strong>重複の扱い</strong>: 同一内容の履歴は最新位置へ移動し、重複して並ばない</li>
<li><strong>クリア</strong>: 「履歴をクリア」でいつでも全件削除</li>
</ul>

### 🔒 セキュリティ

<div style="border: 1px solid #10ac84; border-radius: 8px; padding: 16px; margin: 12px 0; background: rgba(16, 172, 132, 0.06);">

<p>履歴は機密情報を含む可能性があるため、<strong>ディスクへ書き込まない</strong>ことに加え、<strong>メモリ上でも平文を常駐させない</strong>よう多層的に保護しています。メモリ上のみの保持はファイル漏えいを防げますが、プロセスのメモリ空間内に平文が残る・復号した文字列が不要になってもゼロ化されない、といったリスクは残ります。そのため次の対策を講じています。</p>

<table>
<tbody>
<tr>
<td width="28"><strong>💾</strong></td>
<td><strong>ディスク非永続化</strong> — 履歴はファイルやデータベースへ書き込まず、プロセス実行中のメモリ上にのみ存在。再起動すると履歴は引き継がれない</td>
</tr>
<tr>
<td><strong>🔐</strong></td>
<td><strong>メモリ内暗号化</strong> — 起動のたびに <code>getrandom</code> で生成したセッション鍵 (32 バイト) で各エントリを <code>ChaCha20-Poly1305</code> により暗号化。エントリごとにランダムなノンスを付与し、保持するのは暗号文のみ</td>
</tr>
<tr>
<td><strong>#️⃣</strong></td>
<td><strong>平文の非保持</strong> — 重複判定は <code>BLAKE3</code> ハッシュのみで行い、履歴ストア内に加工後テキストの平文は格納しない</td>
</tr>
<tr>
<td><strong>🧹</strong></td>
<td><strong>復号時のゼロ化</strong> — 履歴の呼び出しや加工の取り消しで一時的に復号した文字列は <code>Zeroizing</code> 型 (<code>SecretString</code>) で保持し、スコープを抜けるとメモリをゼロ化</td>
</tr>
<tr>
<td><strong>🗑️</strong></td>
<td><strong>終了時の破棄</strong> — プロセス終了時に暗号鍵を <code>zeroize</code> し、暗号文バッファもクリア。鍵は再起動後も復元できない</td>
</tr>
<tr>
<td><strong>🙈</strong></td>
<td><strong>表示マスキング</strong> — 履歴メニューのラベルは API キー・JWT・秘密鍵など機密らしい内容を <code>[機密情報のため非表示]</code> に置換 (UI 上の漏えいを抑制)</td>
</tr>
</tbody>
</table>

<p>加工の取り消し用に保持する直近 1 件の加工前テキストも、同様にメモリ上のみ・<code>SecretString</code> によるゼロ化付きで管理します。</p>

<div style="border-left: 4px solid #feca57; padding: 10px 14px; margin: 12px 0 0; background: rgba(254, 202, 87, 0.08); border-radius: 0 6px 6px 0;">
<strong>補足:</strong> OS のメモリスワップや他プロセスからのメモリダンプなど、ランタイム環境に依存するリスクまでは完全には防げません。履歴機能は必要な場合のみ有効化し、機密性の高い内容は履歴をオフにするか、使用後に「履歴をクリア」することを推奨します。
</div>

</div>

---

## 🚀 ログイン時の自動起動

<p>トレイメニューの「ログイン時に起動」から、OS ログイン時に ClipRefiner を自動起動するかどうかを切り替えられます。各プラットフォームのネイティブ機構を使用します。</p>

| プラットフォーム         | 仕組み                                                                        |
| :----------------------- | :---------------------------------------------------------------------------- |
| <strong>Windows</strong> | 現在のユーザー向けレジストリ <code>Run</code> キー                            |
| <strong>macOS</strong>   | <code>~/Library/LaunchAgents/</code> への LaunchAgent 配置                    |
| <strong>Linux</strong>   | XDG <code>~/.config/autostart/</code> への <code>.desktop</code> ファイル配置 |

<div style="border-left: 4px solid #feca57; padding: 10px 14px; margin: 12px 0; background: rgba(254, 202, 87, 0.08); border-radius: 0 6px 6px 0;">
⚠️ 実行ファイルのパスが変わった場合 (再インストールや移動後など) は、一度オフにしてから再度オンにすると正しいパスで再登録されます。
</div>

---

## 📝 加工モードの使用例

<details>
<summary><strong>UTM パラメータの削除</strong> (<code>remove-utm</code>)</summary>
<br>

|                       |                                                                                   |
| :-------------------- | :-------------------------------------------------------------------------------- |
| <strong>入力</strong> | <code>https://example.com/page?id=123&utm_source=twitter&utm_medium=social</code> |
| <strong>出力</strong> | <code>https://example.com/page?id=123</code>                                      |

</details>

<details>
<summary><strong>Excel から Markdown へ</strong> (<code>excel-to-markdown</code>)</summary>
<br>

<strong>入力 (TSV):</strong>

```
ID	Name	Price
1	Apple	150
2	Banana	100
```

<strong>出力:</strong>

```markdown
| ID  | Name   | Price |
| --- | ------ | ----- |
| 1   | Apple  | 150   |
| 2   | Banana | 100   |
```

</details>

<details>
<summary><strong>正規表現置換</strong> (<code>regex-replace</code>)</summary>
<br>

<strong>設定 (<code>config.toml</code>):</strong>

```toml
[regex]
pattern = "(\\d{4})-(\\d{2})-(\\d{2})"
replacement = "$1/$2/$3"
case_insensitive = false
multiline = false
```

|                       |                                                   |
| :-------------------- | :------------------------------------------------ |
| <strong>入力</strong> | <code>会議日: 2024-01-15、締切: 2024-02-28</code> |
| <strong>出力</strong> | <code>会議日: 2024/01/15、締切: 2024/02/28</code> |

</details>

<details>
<summary><strong>タイムスタンプ変換</strong> (<code>timestamp-to-datetime</code>)</summary>
<br>

|                       |                                  |
| :-------------------- | :------------------------------- |
| <strong>入力</strong> | <code>1700000000</code>          |
| <strong>出力</strong> | <code>2023-11-14 22:13:20</code> |

</details>

<details>
<summary><strong>ケース変換</strong> (<code>to-snake-case</code>)</summary>
<br>

|                       |                          |
| :-------------------- | :----------------------- |
| <strong>入力</strong> | <code>HelloWorld</code>  |
| <strong>出力</strong> | <code>hello_world</code> |

</details>

<details>
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

|                       |                        |
| :-------------------- | :--------------------- |
| <strong>入力</strong> | <code>1234567</code>   |
| <strong>出力</strong> | <code>1,234,567</code> |

</details>

---

## 📦 インストール

<p>リリース済みのインストーラーがある場合は、各プラットフォーム向けのパッケージを利用する。ソースからビルドする手順は <a href="DEVELOPMENT.md">DEVELOPMENT.md</a> を参照。</p>

### Windows (MSI)

<p>配布用 MSI を実行し、画面の指示に従ってインストールする。per-user インストールのため、管理者権限は不要。</p>

### macOS (DMG)

<p>DMG を開き、<code>ClipRefiner.app</code> を Applications フォルダへドラッグする。初回起動時に Gatekeeper の確認が表示される場合がある。</p>

### Linux (deb)

```bash
sudo dpkg -i clip-refiner_{version}-1_{arch}.deb
```

<p>インストール後は <code>ClipRefiner</code> コマンドまたはアプリケーションメニューから起動できる。</p>

<div style="border-left: 4px solid #feca57; padding: 10px 14px; margin: 12px 0; background: rgba(254, 202, 87, 0.08); border-radius: 0 6px 6px 0;">
<strong>ポータブル利用:</strong> インストーラーを使わず単体の実行ファイルだけを配置する場合も、引数なしで起動すればシステムトレイに常駐して動作する (設定はユーザの設定ディレクトリへ保存される)
</div>

---

## ⚙️ 設定

<p>設定は <code>config.toml</code> に自動保存されます。トレイメニューの「設定を開く」から編集するか、次の場所のファイルを直接編集できます。</p>

<table>
<tbody>
<tr>
<td width="140"><strong>Windows</strong></td>
<td><code>%APPDATA%\ClipRefiner\config.toml</code></td>
</tr>
<tr>
<td><strong>Linux / macOS</strong></td>
<td><code>~/.config/clip-refiner/config.toml</code></td>
</tr>
</tbody>
</table>

<p>設定項目の一覧、ホットキー形式、加工パイプライン、監視方式、処理上限、ログの保存場所などは <strong><a href="CONFIG.md">CONFIG.md</a></strong> (設定リファレンス) にまとめています。</p>

---

<div align="center">

## 📄 ライセンス

<p>
<a href="LICENSE">All Rights Reserved</a>
</p>

</div>
