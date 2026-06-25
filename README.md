<div align="center">

<img src="assets/icon.png" width="128" height="128" alt="ClipRefiner Logo">

<h1>ClipRefiner</h1>

<p>
  <strong>クリップボードのテキストをリアルタイムで監視し、指定した形式に自動加工するデスクトップツール</strong>
</p>

<p>
  <img src="https://img.shields.io/badge/Rust-1.96%20%7C%202024_edition-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPldpbmRvd3M8L3RpdGxlPjxwYXRoIGQ9Ik0zIDEyLjVWNi44bDgtMS4xdjYuOEgzem05LTcuMyAxMC0xLjR2OC43SDEyVjUuMnpNMyAxMy41aDh2NS43bC04LTEuMnYtNC41em05IDBoMTB2OC42bC0xMC0xLjR2LTcuMnoiLz48L3N2Zz4%3D&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux">
  <img src="https://img.shields.io/badge/License-All%20Rights%20Reserved-F59E0B?style=for-the-badge&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiIHJvbGU9ImltZyI%2BPHRpdGxlPkNvcHlyaWdodDwvdGl0bGU%2BPHBhdGggZD0iTTEyIDJhMTAgMTAgMCAxIDAgMCAyMCAxMCAxMCAwIDAgMCAwLTIwem0wIDJhOCA4IDAgMSAxIDAgMTYgOCA4IDAgMCAxIDAtMTZ6bS0xIDQuNWMtMi4yIDAtMy41IDEuNi0zLjUgMy41czEuMyAzLjUgMy41IDMuNWMxLjEgMCAyLS41IDIuNi0xLjJsLTEuMi0xLjJjLS40LjUtMSAuOC0xLjYuOC0xLjIgMC0yLS45LTItMnMuOC0yIDItMmMuNiAwIDEuMS4yIDEuNS42bDEuMi0xLjJjLS43LS43LTEuNy0xLjEtMi45LTEuMXoiLz48L3N2Zz4%3D&logoColor=white" alt="License">
</p>

<p>
  <sub>36 種類の加工モード &middot; グローバルホットキー &middot; 登録文字列 &middot; 暗号化履歴 &middot; 機密情報マスキング</sub>
</p>

</div>

---

## 📌 目次

<table>
<tr>
<td valign="top" width="50%">

- [✨ 主な機能](#-主な機能)
- [🛠️ 加工モード一覧](#️-加工モード一覧)
- [🚀 使用方法](#-使用方法)
- [🖥️ システムトレイメニュー](#️-システムトレイメニュー)
- [🪟 クイックセレクタ](#-クイックセレクタ)
- [📋 登録文字列セレクタ](#-登録文字列セレクタ)
- [⌨️ グローバルホットキー](#️-グローバルホットキー)

</td>
<td valign="top" width="50%">

- [↩️ 加工の取り消し](#️-加工の取り消し)
- [🕘 クリップボード履歴](#-クリップボード履歴)
- [🚀 ログイン時の自動起動](#-ログイン時の自動起動)
- [📝 加工モードの使用例](#-加工モードの使用例)
- [🛠️ インストール・ビルド](#️-インストールビルド)
- [⚙️ 設定](#️-設定)
- [📋 ログ](#-ログ)
- [📄 ライセンス](#-ライセンス)

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
<strong>📋 登録文字列</strong><br>
よく使うテキストを保存し、ホットキーまたはトレイから即コピー
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
通知・履歴メニュー・登録文字列プレビューで API キーや JWT などを非表示
</div>

<div style="border-left: 4px solid #5f27cd; padding: 12px 16px; margin: 8px 0; background: rgba(95, 39, 205, 0.08); border-radius: 0 8px 8px 0;">
<strong>⌨️ グローバルホットキー</strong><br>
どのウィンドウからでもキー操作で機能を呼び出し
</div>

<div style="border-left: 4px solid #ee5253; padding: 12px 16px; margin: 8px 0; background: rgba(238, 82, 83, 0.08); border-radius: 0 8px 8px 0;">
<strong>↩️ 加工の取り消し</strong><br>
直近の加工をホットキーで元のテキストへ復元
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
<strong>全 36 モード</strong>に対応しています。
</p>

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
<td rowspan="3"><strong>マークダウン</strong></td>
<td><code>markdown-to-html</code></td>
<td>Markdown を HTML へ変換</td>
</tr>
<tr>
<td><code>excel-to-markdown</code></td>
<td>Excel コピーデータを Markdown テーブルへ変換</td>
</tr>
<tr>
<td><code>markdown-to-excel</code></td>
<td>Markdown 表を Excel (TSV) 形式へ変換</td>
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
</tbody>
</table>

<div style="border: 1px solid #4a9eff; border-radius: 8px; padding: 12px 16px; margin: 16px 0; background: rgba(74, 158, 255, 0.06);">
💡 <strong>ヒント:</strong> 正規表現モードは <code>config.toml</code> の <code>[regex]</code> セクションでパターン・置換文字列・オプションを設定します。各モードの入出力例は <a href="#-加工モードの使用例">加工モードの使用例</a> を参照してください。
</div>

---

## 🚀 使用方法

### 監視モード (常駐)

<p>引数なしで実行すると、システムトレイ (通知領域) にアイコンが表示され、クリップボードの監視を開始します。アイコンの右クリックメニューから加工モードの切り替えや監視の一時停止などが行えます。</p>

```bash
./ClipRefiner.exe
```

### ワンショットモード

<p>特定の加工を一度だけ行いたい場合は <code>--mode</code> (短縮形 <code>-m</code>) でモードを指定します。常駐せずに、現在のクリップボードの内容を加工して書き戻し、すぐに終了します。</p>

```bash
# クリップボード内の URL をデコードする
./ClipRefiner.exe --mode url-decode

# 短縮形でも指定できる
./ClipRefiner.exe -m json-format

# 正規表現で置換 (config.toml の [regex] を使用)
./ClipRefiner.exe -m regex-replace

# 正規表現設定を CLI で上書き (ワンショット時のみ)
./ClipRefiner.exe -m regex-replace --regex-pattern "(\d{4})-(\d{2})-(\d{2})" --regex-replacement "$1/$2/$3"
```

### コマンドラインオプション

| オプション                                        | 説明                                                                                    |
| :------------------------------------------------ | :-------------------------------------------------------------------------------------- |
| <code>-m</code>, <code>--mode &lt;MODE&gt;</code> | ワンショットで実行する加工モードを指定 ([加工モード一覧](#️-加工モード一覧) 参照)        |
| <code>--regex-pattern &lt;PATTERN&gt;</code>      | 正規表現パターン (<code>config.toml</code> の <code>regex.pattern</code> を上書き)      |
| <code>--regex-replacement &lt;TEXT&gt;</code>     | 置換文字列 (<code>regex.replacement</code> を上書き。<code>regex-replace</code> で使用) |
| <code>--regex-case-insensitive</code>             | 大文字小文字を無視 (<code>(?i)</code> 相当)                                             |
| <code>--regex-multiline</code>                    | 複数行モード (<code>(?m)</code> 相当)                                                   |
| <code>-h</code>, <code>--help</code>              | ヘルプを表示                                                                            |
| <code>-V</code>, <code>--version</code>           | バージョンを表示                                                                        |

<p>正規表現オプションはワンショット実行時のみ有効です。常駐モードでは <code>config.toml</code> の <code>[regex]</code> セクションが使用されます。</p>

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
<td>加工モードをカテゴリ別のサブメニューから選択。現在のモードにはチェックが付く</td>
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
<td><strong>登録文字列</strong></td>
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
<li><strong>検索</strong>: モード名・カテゴリ・CLI 名 (<code>--mode</code> の値) のいずれにも部分一致で絞り込み</li>
<li><strong>現在のモード</strong>: 表示時に現在選択中のモードがハイライト</li>
<li><strong>マウス操作</strong>: ホバー選択・クリック決定にも対応</li>
</ul>

| キー                             | 動作                                               |
| :------------------------------- | :------------------------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>      | 候補の移動                                         |
| <kbd>Home</kbd> / <kbd>End</kbd> | 先頭 / 末尾へ移動                                  |
| <kbd>Enter</kbd>                 | 選択中のモードを決定                               |
| <kbd>Esc</kbd>                   | 検索文字列があればクリア、空ならウィンドウを閉じる |

---

## 📋 登録文字列セレクタ

<p>よく使うテキストを <code>config.toml</code> に登録し、ホットキーまたはトレイメニューからクリップボードへ即コピーできる機能です。クイックセレクタと同様のコマンドパレット風 UI (<strong>登録文字列セレクタ</strong>) も利用できます。</p>

<div style="border-left: 4px solid #ff9f43; padding: 12px 16px; margin: 12px 0; background: rgba(255, 159, 67, 0.08); border-radius: 0 8px 8px 0;">
<strong>表示:</strong> グローバルホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd>)<br>
<strong>登録:</strong> トレイメニュー「登録文字列」→「クリップボードを登録」、またはセレクタ内で <kbd>Ctrl</kbd> + <kbd>Enter</kbd><br>
<strong>上限:</strong> 最大 100 件、ラベル 64 文字、本文はクリップボード上限 (2 MiB) まで
</div>

| キー                               | 動作                                               |
| :--------------------------------- | :------------------------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>        | 候補の移動                                         |
| <kbd>Enter</kbd>                   | 選択中の文字列をクリップボードへコピー             |
| <kbd>Del</kbd>                     | 選択中の登録文字列を削除                           |
| <kbd>Ctrl</kbd> + <kbd>Enter</kbd> | 現在のクリップボード内容を新規登録                 |
| <kbd>Esc</kbd>                     | 検索文字列があればクリア、空ならウィンドウを閉じる |

<p>登録文字列は <code>config.toml</code> の <code>[[texts]]</code> セクションに永続化されます。ラベルは本文の先頭から自動生成されます。</p>

```toml
[[texts]]
label = "挨拶文"
text = "お疲れ様です。よろしくお願いいたします。"
```

---

## ⌨️ グローバルホットキー

<p>監視モード常駐時に、アクティブなウィンドウを問わず以下のホットキーが使用できます (<code>config.toml</code> の <code>hotkeys</code> で変更可能。反映には再起動が必要)。</p>

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
<td>登録文字列セレクタの表示・非表示</td>
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
</tbody>
</table>

---

## ↩️ 加工の取り消し

<p>監視モードで加工が成功した直後のみ、直近 1 件分の取り消しが可能です。ホットキー (既定で <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Z</kbd>) またはトレイメニューの「ショートカット一覧」から割り当てを確認できます。</p>

<ul>
<li><strong>対象</strong>: 監視モードでの自動加工、または手動で実行した加工のうち、直前に成功した 1 件</li>
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
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

|                       |                        |
| :-------------------- | :--------------------- |
| <strong>入力</strong> | <code>1234567</code>   |
| <strong>出力</strong> | <code>1,234,567</code> |

</details>

---

## 🛠️ インストール・ビルド

### 前提条件

<ul>
<li><a href="https://www.rust-lang.org/tools/install">Rust / Cargo</a> (edition 2024、Rust 1.96 以上。<code>rust-toolchain.toml</code> でピン留め)</li>
</ul>

#### Linux の追加パッケージ

<p>GUI および通知機能のために、以下のパッケージが必要になる場合があります:</p>

```bash
sudo apt-get install libdbus-1-dev pkg-config libatk1.0-dev libgtk-3-dev
```

### ビルド

```bash
git clone <repository_url>
cd clip-refiner
cargo build --release
```

<p>バイナリは <code>target/release/ClipRefiner</code> (Windows では <code>ClipRefiner.exe</code>) に生成されます。各プラットフォーム用のビルドスクリプト (<code>scripts/windows/build.ps1</code>、<code>scripts/macos/build.sh</code>、<code>scripts/linux/build.sh</code>) も利用できます。</p>

### プラットフォーム別インストーラー (任意)

#### Windows MSI

<p><code>cargo-wix</code> と WiX Toolset v3 を使って MSI を作成できます。</p>

```powershell
# 前提: cargo install cargo-wix --locked
#       winget install WiXToolset.WiXToolset
./scripts/windows/build-msi.ps1
```

<p>出力先: <code>target/wix/clip-refiner-{version}-{arch}.msi</code> (per-user インストール、日本語 UI)</p>

#### macOS DMG

<p><code>cargo-bundle</code> で <code>.app</code> バンドルを作成し、<code>hdiutil</code> で DMG インストーラーを生成します。macOS 11.0 以降が必要です。</p>

```bash
# 前提: cargo install cargo-bundle --locked
#       Xcode Command Line Tools
./scripts/macos/build-dmg.sh
```

<p>出力先:</p>

<ul>
<li><code>target/release/bundle/osx/ClipRefiner.app</code></li>
<li><code>target/bundle/clip-refiner-{version}-{arch}.dmg</code> (Applications へのシンボリックリンク付き)</li>
</ul>

<p><code>--skip-build</code> (<code>-s</code>) を付けると、既存のリリースビルドからパッケージのみ作成します。</p>

#### Linux deb

<p><code>cargo-deb</code> で <code>.deb</code> パッケージを作成します。デスクトップエントリとアイコンも同梱されます。</p>

```bash
# 前提: cargo install cargo-deb --locked
#       上記「Linux の追加パッケージ」をインストール済みであること
./scripts/linux/build-deb.sh
```

<p>出力先: <code>target/debian/clip-refiner_{version}-1_{arch}.deb</code></p>

<p>インストール後は <code>ClipRefiner</code> コマンドとアプリケーションメニューから起動できます。ログイン時自動起動は XDG <code>autostart</code> を使用します。</p>

<p><code>--skip-build</code> (<code>-s</code>) を付けると、既存のリリースビルドからパッケージのみ作成します。</p>

---

## ⚙️ 設定

<p>設定ファイル (<code>config.toml</code>) は設定変更のたびに自動保存され、以下の場所に配置されます。</p>

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

<div style="border-left: 4px solid #7c5cff; padding: 10px 14px; margin: 12px 0; background: rgba(124, 92, 255, 0.08); border-radius: 0 6px 6px 0;">
<strong>設定ディレクトリ名:</strong> Windows は <code>ClipRefiner</code>、Linux/macOS は <code>clip-refiner</code> (OS ごとの慣例に合わせた名称)
</div>

<p>設定ファイルの解析に失敗した場合、元ファイルは <code>config.toml.bak</code> として退避され、デフォルト設定で起動します。TOML 形式のため <code>#</code> でコメントを書けます。初回保存時は各項目の説明コメントが付与され、以降の保存ではユーザーが追記したコメントを維持したまま値のみ更新されます。設定ディレクトリとログファイルは、Unix では所有者専用パーミッション、Windows では現在ユーザー専用 DACL で保護されます。</p>

### 処理の制限

| 対象                                | 上限                    |
| :---------------------------------- | :---------------------- |
| クリップボード本文                  | 2 MiB                   |
| JSON / YAML / Markdown パーサー入力 | 1 MiB                   |
| 正規表現パターン                    | 8 KiB                   |
| 登録文字列                          | 100 件 (ラベル 64 文字) |

<p>上限を超える入力は処理されず、登録文字列の追加は拒否されます。通知・履歴メニュー・登録文字列プレビューでは、API キー・JWT・PEM 秘密鍵・資格情報行など機密らしい内容を <code>[機密情報のため非表示]</code> に自動置換します (クリップボード本体は加工対象のまま保持)。</p>

### 設定項目

<table>
<thead>
<tr>
<th align="left">キー</th>
<th align="left">型</th>
<th align="left">デフォルト</th>
<th align="left">説明</th>
</tr>
</thead>
<tbody>
<tr><td><code>version</code></td><td>number</td><td><code>0</code></td><td>設定スキーマのバージョン</td></tr>
<tr><td><code>mode</code></td><td>string</td><td><code>"UrlDecode"</code></td><td>使用する加工モード</td></tr>
<tr><td><code>interval_ms</code></td><td>number</td><td><code>1000</code></td><td>クリップボードのポーリング間隔 (ミリ秒、100〜60000)</td></tr>
<tr><td><code>monitor_mode</code></td><td>string</td><td><code>"Polling"</code></td><td>監視方式。<code>"Polling"</code> または <code>"Event"</code></td></tr>
<tr><td><code>is_paused</code></td><td>bool</td><td><code>false</code></td><td>監視を一時停止するかどうか</td></tr>
<tr><td><code>history_enabled</code></td><td>bool</td><td><code>false</code></td><td>加工履歴の有効・無効</td></tr>
<tr><td><code>history_limit</code></td><td>number</td><td><code>10</code></td><td>履歴の最大保持件数 (1〜100)</td></tr>
<tr><td><code>notification_settings.enabled</code></td><td>bool</td><td><code>false</code></td><td>デスクトップ通知の有効・無効</td></tr>
<tr><td><code>notification_settings.notify_mode</code></td><td>bool</td><td><code>true</code></td><td>モード変更時の通知</td></tr>
<tr><td><code>notification_settings.notify_result</code></td><td>bool</td><td><code>false</code></td><td>通知にクリップボードの内容を表示するかどうか</td></tr>
<tr><td><code>notification_settings.notify_pause</code></td><td>bool</td><td><code>true</code></td><td>一時停止切替時の通知</td></tr>
<tr><td><code>hotkeys.quick_selector</code></td><td>string</td><td><code>"Alt+Shift+S"</code></td><td>クイックセレクター表示</td></tr>
<tr><td><code>hotkeys.text_selector</code></td><td>string</td><td><code>"Alt+Shift+T"</code></td><td>登録文字列セレクター表示</td></tr>
<tr><td><code>hotkeys.notification</code></td><td>string</td><td><code>"Alt+Shift+N"</code></td><td>成功通知の ON/OFF</td></tr>
<tr><td><code>hotkeys.pause</code></td><td>string</td><td><code>"Alt+Shift+P"</code></td><td>監視の一時停止・再開</td></tr>
<tr><td><code>hotkeys.undo</code></td><td>string</td><td><code>"Alt+Shift+Z"</code></td><td>直近の加工を取り消し</td></tr>
<tr><td><code>hotkeys.quit</code></td><td>string</td><td><code>"Alt+Shift+Q"</code></td><td>アプリケーション終了</td></tr>
<tr><td><code>regex.pattern</code></td><td>string</td><td><code>""</code></td><td>正規表現パターン (最大 8 KiB)</td></tr>
<tr><td><code>regex.replacement</code></td><td>string</td><td><code>""</code></td><td>置換文字列 (<code>regex-replace</code> で使用。<code>$1</code> 形式のキャプチャ参照可)</td></tr>
<tr><td><code>regex.case_insensitive</code></td><td>bool</td><td><code>false</code></td><td>大文字小文字を無視 (<code>(?i)</code> 相当)</td></tr>
<tr><td><code>regex.multiline</code></td><td>bool</td><td><code>false</code></td><td>複数行モード (<code>(?m)</code> 相当)</td></tr>
<tr><td><code>[[texts]]</code></td><td>array</td><td>(空)</td><td>登録文字列 (<code>label</code> / <code>text</code>)。最大 100 件</td></tr>
</tbody>
</table>

### ホットキー形式

<p><code>Alt+Shift+S</code> のように、<code>+</code> 区切りで修飾キーとキーを指定します。</p>

<ul>
<li><strong>修飾キー</strong>: <code>Alt</code>, <code>Shift</code>, <code>Ctrl</code> (<code>Control</code> 可), <code>Meta</code> (<code>Super</code> / <code>Win</code> 可)</li>
<li><strong>キー</strong>: <code>A</code>〜<code>Z</code>, <code>F1</code>〜<code>F12</code></li>
</ul>

<p>不正な値は起動時にデフォルトへ置き換えられます。変更を反映するにはアプリの再起動が必要です。</p>

### 監視方式 (<code>monitor_mode</code>)

| 方式                 | 説明                                                                                                                                                                                                                              |
| :------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>Polling</code> | 一定間隔 (<code>interval_ms</code>) でクリップボードの内容を読み取り、変更を検知。すべてのプラットフォームで動作する基本方式                                                                                                      |
| <code>Event</code>   | OS の変更トークン (Windows: シーケンス番号、macOS: <code>changeCount</code>、Linux: X11 の CLIPBOARD オーナー / Wayland の data-control 選択イベント) を監視。本文の定期読み取りを避けるため、ポーリングより低遅延かつ低 CPU 負荷 |

<div style="border-left: 4px solid #ee5253; padding: 10px 14px; margin: 12px 0; background: rgba(238, 82, 83, 0.08); border-radius: 0 6px 6px 0;">
<strong>Linux での注意:</strong> Wayland では <code>ext-data-control-v1</code> または <code>wlr-data-control-unstable-v1</code> に対応した compositor (GNOME、KDE、Sway、Hyprland など) で <code>Event</code> 方式が利用できます。いずれのバックエンドも利用できない環境では、自動的にポーリングへフォールバックします。
</div>

---

## 📋 ログ

<p>ログファイルは設定ディレクトリ内の <code>logs/</code> フォルダに日次ローテーションで保存されます。</p>

<table>
<tbody>
<tr>
<td width="140"><strong>Windows</strong></td>
<td><code>%APPDATA%\ClipRefiner\logs\</code></td>
</tr>
<tr>
<td><strong>Linux / macOS</strong></td>
<td><code>~/.config/clip-refiner/logs/</code></td>
</tr>
</tbody>
</table>

<p>ログレベルは環境変数 <code>RUST_LOG</code> で制御できます (例: <code>RUST_LOG=debug</code>)。</p>

---

<div align="center">

## 📄 ライセンス

<p>
<a href="LICENSE">All Rights Reserved</a>
</p>

<p>
<sub>Made with Rust 🦀</sub>
</p>

</div>
