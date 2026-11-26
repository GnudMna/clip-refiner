# 変換モード

変換モードの一覧と使用例をまとめる。監視で適用するモードの指定は [設定リファレンス](configuration.md#設定項目)、複数モードの連鎖は [加工パイプライン](configuration.md#加工パイプライン-pipeline) を参照。

## 変換モード一覧

`--mode` に渡す値 (CLI 名) と、トレイメニュー・クイックセレクターでの表示名。カテゴリはトレイメニューのサブメニューとクイックセレクターの見出しに対応する。**全 55 モード**に対応。

> **設定ファイル:** CLI は kebab-case (`url-decode` など) だが、`config.toml` では PascalCase (`UrlDecode` など) を指定する。初回保存時に生成される `config.toml` を参照するとよい。

| カテゴリ | モード名 (`--mode`) | 表示名 | 説明 |
| :--- | :--- | :--- | :--- |
| **URL操作** | `url-encode` / `url-decode` | URLエンコード / URLデコード | URL のエンコード・デコード (エンコードは英数字と `-` `.` `_` `~` 以外の文字が対象。デコードは `+` を変えない) |
| | `remove-utm` | UTM除去 | URL から `utm_*` と `fbclid` / `gclid` などの計測パラメータを削除 |
| **Base64** | `base64-encode` / `base64-decode` | Base64エンコード / Base64デコード | UTF-8 の文字列と Base64 (標準アルファベット・パディングあり) の相互変換。画像にも対応する ([Base64 と画像](#base64-と画像)) |
| **ファイルパス** | `extract-basename` / `extract-basename-quoted` | ベースネーム抽出 / ベースネーム抽出(引用符付) | パスからファイル名のみを抽出 (引用符付きオプションあり) |
| | `add-path-quotes` / `remove-path-quotes` | 引用符を付与 / 引用符を削除 | パスへの引用符 (`"`) の付与・削除 |
| | `path-to-slash` / `path-to-backslash` | スラッシュ区切りに変換 / バックスラッシュ区切りに変換 | パス区切り文字をスラッシュ / バックスラッシュに変換 |
| **行の並べ替え・削除** | `sort-lines-asc` / `sort-lines-desc` | 昇順で並び替え / 降順で並び替え | 行単位での昇順・降順ソート (クォート内改行を含む 2 列以上の CSV はレコード単位) |
| | `sort-lines-natural-asc` / `sort-lines-natural-desc` | 昇順で並び替え(自然順) / 降順で並び替え(自然順) | 連続する数字を数値として比較する自然順ソート (`2` は `10` より前。それ以外は `sort-lines-asc` と同じ) |
| | `remove-empty-lines` | 空行削除 | 空行を削除 |
| | `remove-duplicate-lines` | 重複行削除 | 重複行を削除 (クォート内改行を含む 2 列以上の CSV はレコード単位) |
| **空白の除去** | `trim` | 全体をトリム | テキスト全体の前後の空白・改行を削除 (BOM・ゼロ幅スペースを含む) |
| | `trim-lines` | 行単位でトリム | 行ごとに前後の空白 (全角・BOM・ゼロ幅スペースを含む) を削除 |
| **エスケープ・解除** | `escape` / `unescape` | エスケープ / アンエスケープ | バックスラッシュエスケープの付与・解除 ([エスケープの対象](#エスケープの対象)) |
| | `regex-escape` / `regex-unescape` | 正規表現エスケープ / 正規表現アンエスケープ | 正規表現メタ文字のエスケープ・解除 ([エスケープの対象](#エスケープの対象)) |
| **正規表現** | `regex-replace` | 正規表現置換 | パターンに一致する部分を置換 (`[regex]` 設定を使用) |
| | `regex-extract` | 正規表現抽出 | パターンに一致する部分を行単位で抽出 |
| | `regex-delete` | 正規表現削除 | パターンに一致する部分を削除 |
| | `regex-split` | 正規表現分割 | パターンで分割し改行で結合 |
| **JSON整形** | `json-format` | JSON整形(キー順序ソート) | JSON をインデント整形 (キーを辞書順にソート) |
| | `json-format-preserve-order` | JSON整形(キー順序保持) | JSON をインデント整形 (キー順序保持) |
| **YAML→JSON** | `yaml-to-json` | YAML→JSON(キー順序不同) | YAML を JSON へ変換 (キー順序不同) |
| | `yaml-to-json-preserve-order` | YAML→JSON(キー順序保持) | YAML を JSON へ変換 (キー順序保持) |
| **JSON→YAML** | `json-to-yaml` | JSON→YAML(キー順序ソート) | JSON を YAML へ変換 (キーを辞書順にソート) |
| | `json-to-yaml-preserve-order` | JSON→YAML(キー順序保持) | JSON を YAML へ変換 (キー順序保持) |
| **Markdown・HTML** | `markdown-to-html` | Markdown→HTML | Markdown を HTML ソース (テキスト) へ変換 |
| | `markdown-to-rich-text` | Markdown→リッチテキスト | Markdown をリッチテキスト (HTML 形式) としてクリップボードへ書き込む |
| | `html-to-markdown` | HTML→Markdown | HTML を Markdown へ変換 (ブラウザでコピーした HTML 形式を優先) |
| **Excel** | `excel-to-markdown` | Excel→Markdown | Excel コピーデータを Markdown テーブルへ変換 |
| | `markdown-to-excel` | Markdown→Excel | Markdown 表を Excel (TSV) 形式へ変換 (区切り行 `\|---\|` のある表だけを変換し、表以外の行はそのまま残す) |
| | `excel-to-image` | Excel→画像 | Excel コピーデータの見た目を画像としてクリップボードへ保存 (エクスプローラへもファイルとして貼り付けられる。[画像のファイル貼り付け](#画像のファイル貼り付け)) |
| **日時変換** | `to-datetime-string` / `to-unix-timestamp` | 日時→日時文字列 / 日時→Unixタイムスタンプ | 日時 ↔ Unix タイムスタンプ・日時文字列の変換。入力の形式は行ごとに自動判定 (ローカルタイムゾーン。複数行対応) |
| | `to-datetime-string-utc` | 日時→日時文字列(UTC) | 日時を UTC の日時文字列へ変換 |
| | `to-iso8601` / `to-iso8601-utc` | 日時→ISO 8601 / 日時→ISO 8601(UTC) | 日時を ISO 8601 (RFC 3339) 形式へ変換 (ローカルタイムゾーンのオフセット付き / UTC) |
| | `to-rfc2822` | 日時→RFC 2822 | 日時を RFC 2822 形式へ変換 (ローカルタイムゾーン) |
| | `to-japanese-datetime` | 日時→和文日時 | 日時を和文形式 (`2023年11月15日 07時13分20秒`) へ変換 (ローカルタイムゾーン) |
| **桁区切りカンマ** | `add-comma` / `remove-comma` | カンマ追加 / カンマ除去 | 数値への 3 桁カンマ区切り付与・削除 (複数行対応。先頭の符号 `-` / `+` と小数に対応。`00123456` のように先頭がゼロの整数は ID・電話番号などとみなし、カンマ追加では変換しない) |
| **識別子の書式** | `to-camel-case` / `to-snake-case` / `to-pascal-case` / `to-kebab-case` / `to-screaming-snake-case` | camelCaseへ変換 / snake_caseへ変換 / PascalCaseへ変換 / kebab-caseへ変換 / SCREAMING_SNAKE_CASEへ変換 | 識別子のケース変換 (行頭のインデントと、先頭の `_` / `-` (`_private`・`--dry-run` など) は維持) |
| **全角・半角** | `to-halfwidth` / `to-fullwidth` | 半角へ変換 / 全角へ変換 | 英数字・記号・スペース・カタカナの全角 ↔ 半角変換 ([全角・半角変換の対象](#全角半角変換の対象)) |

## モードごとの仕様

一覧の説明だけでは足りないモードの、対象・判定の条件・変換しない入力をまとめる。具体的な入出力は [変換モードの使用例](#変換モードの使用例) を参照。

### ファイルパスの判定

ファイルパスのモードは、行ごとに次のいずれかで始まる行だけをパスとして扱い、それ以外の行は変更しない。URL (`https://…`) や `and/or`・`TCP/IP` のように区切り文字を含むだけの文章、`12:00:00` のような時刻は対象外である。引用符で囲まれた行は、囲みを除いた内容で判定する。`// comment` のように `//` や `\\` の直後が空白の行はコメントとみなし、パスとして扱わない。

パスと判定した行は行全体を変換する。引用符の付与・削除 (`add-path-quotes` / `remove-path-quotes`) と区切り文字の変換 (`path-to-slash` / `path-to-backslash`) では、行頭のインデントと行末の空白を維持する。`/usr/bin and/or` のようにスペースを含む行も、ディレクトリ名にスペースを含むパスと区別できないため行全体を変換する。

| 始まり | 例 |
| :--- | :--- |
| ドライブレター | `C:\Users\sample`, `D:/work` |
| `/` または `\` (UNC を含む) | `/usr/local/bin`, `\\server\share` |
| `./` `../` `.\` `..\` (相対パス) | `./src/main.rs`, `..\docs` |

### 改行コード

行単位で処理するモード (`sort-lines-*` / `remove-empty-lines` / `remove-duplicate-lines` / `trim-lines` / 識別子のケース変換) は、CRLF と LF が混在するテキストでも行ごとの改行コードを変えない。並び替えでは改行コードの並びを位置ごとに保つ。`markdown-to-excel` も表以外の行の改行コードと末尾の改行を変えない ([Excel と Markdown の表](#excel-と-markdown-の表))。

### エスケープの対象

`escape` は、JSON の文字列としてそのまま使える形にする。`/` や `'`、日本語などの ASCII 以外の文字は変えない。

| 元の文字 | `escape` の出力 |
| :--- | :--- |
| `"` / `\` | `\"` / `\\` |
| 改行 (LF) / 復帰 (CR) / タブ | `\n` / `\r` / `\t` |
| バックスペース / フォームフィード | `\b` / `\f` |
| 上記以外の制御文字 (U+0000〜U+001F) | `\u0000` 形式 |

`unescape` は、`\"` `\\` `\/` `\b` `\f` `\n` `\r` `\t` と `\uXXXX` (サロゲートペアを含む) を元の文字へ戻す。それ以外のバックスラッシュ (`\x` や `\'`、16 進数が 4 桁続かない `\u` など) はそのまま残す。

`regex-escape` は、正規表現で特別な意味を持つ `\` `.` `+` `*` `?` `(` `)` `|` `[` `]` `{` `}` `^` `$` `#` `&` `-` `~` の前に `\` を付ける。`regex-unescape` は、これらの文字の前にある `\` だけを外す。

### リッチテキストと HTML 形式

`markdown-to-rich-text` と `html-to-markdown` はテキストだけでなく、クリップボードの HTML 形式 (`CF_HTML`) を読み書きする。

- **`markdown-to-rich-text`:** Markdown を HTML へ変換し、HTML 形式としてクリップボードへ書き込む。Word / Outlook / Teams などに貼り付けると、見出し・リスト・表が整形済みで入る。表には罫線を付ける。プレーンテキスト側には変換前の Markdown を残すため、テキストエディタに貼り付けると Markdown のままになる。HTML ソースをテキストとして得たい場合は `markdown-to-html` を使う
- **`html-to-markdown`:** ブラウザや Word からコピーした内容に HTML 形式があれば、テキストより HTML 形式を優先して Markdown へ変換し、テキストとして書き戻す (HTML 形式は消える)。HTML 形式が無い場合はテキストを HTML ソースとして変換する。タグを含まないテキストは変換しない。見出し・段落・強調・リンク・画像・リスト・引用・コードブロック・表に対応し、`script` / `style` などの非表示要素は出力しない。要素の入れ子が 256 階層を超える HTML は変換しない
- **パイプライン:** `html-to-markdown` は先頭に置いたときだけ HTML 形式を読み、`markdown-to-rich-text` はテキスト加工の最終段に置いたときだけ HTML 形式を書き込む。それ以外の位置ではテキスト同士の変換として働く (`markdown-to-rich-text` は `markdown-to-html` と同じ HTML ソースを返す)。例: `--pipeline excel-to-markdown markdown-to-rich-text` で Excel の表を罫線付きの表として貼り付けられる

### Excel と Markdown の表

`excel-to-markdown` と `markdown-to-excel` は、互いの出力を入力に戻すと元の値になるようにセルを変換する。

- **`excel-to-markdown`:** 1 行目を見出しにし、列数はいちばん多い行に合わせる (足りないセルは空)。セルの前後の空白は除く。セル内の改行は `<br>`、`|` は `\|` にする。文字としての `<br>` は `&lt;br>` にし、`markdown-to-excel` で元の `<br>` に戻る。`|` の直前のバックスラッシュも重ねてエスケープするため、`a\|b` のようなセルも GitHub などで列がずれない。改行・タブ・`"` を含むセルを Excel が `"` で囲んだコピーも、囲みを外して読む
- **`markdown-to-excel`:** 見出し行の直後に区切り行 (`|---|`) が続く塊だけを表とみなし、`|` を含まない行で表が終わる。表以外の行はそのまま残し、改行コードと末尾の改行も変えない。セル内の `<br>` (`<br/>`・`<br />` を含む) は改行に、`\|` は `|` に戻す。インラインコード (`` `a|b` ``) 内の `|` も GFM と同じく列の区切りとみなす。コードフェンス (```` ``` ``` ```` / `~~~`) 内の表は変換しない。改行・タブ・`"` を含むセルは、Excel に貼り付けられるよう `"` で囲む

### Base64 と画像

`base64-encode` と `base64-decode` は、文字列に加えて PNG / JPEG / BMP の画像を扱える。

- **`base64-decode`:** デコード結果が画像なら、テキストではなくクリップボードの画像として書き込む。`data:image/png;base64,…` 形式の data URI も受け付ける。画像でも UTF-8 の文字列でもないデータは変更しない。画像として書き込むのは、パイプラインの最終段に置いたときだけで、書き込み後は加工の取り消し (`Alt+Shift+Z`) でデコード前のテキストへ戻せる
- **`base64-encode`:** クリップボードにテキストが無く画像だけがあるとき、その画像を PNG にして Base64 の文字列へ変換する。テキストがある場合 (Excel のコピーなど) はテキストを優先する。先頭に置いたときだけ画像を読む。変換結果の文字列には、パイプラインの後段のモードが続けて作用する
- **自動変換:** 画像だけのコピーは自動変換の対象外のため、`base64-encode` で画像を変換するにはトレイメニュー・クイックセレクター・ホットキーなどから実行する
- **ファイル貼り付け:** 書き込んだ画像は、エクスプローラへもファイルとして貼り付けられる ([画像のファイル貼り付け](#画像のファイル貼り付け))
- **上限:** 辺長 8192 px、16777216 ピクセル (4096×4096) まで。超える画像は変換しない。変換結果の文字列はクリップボード本文の上限 (2 MiB) に収まる必要がある ([処理の制限](configuration.md#処理の制限))

### 画像のファイル貼り付け

画像を書き込む `excel-to-image` と、画像を復元する `base64-decode` は、ビットマップに加えて PNG のファイルもクリップボードへ載せる。ファイルはメモリ上から直接渡し、ディスクへは書き出さない。

- **貼り付け先:** ペイント・Word・ブラウザなど画像を受け取るアプリには画像として、エクスプローラのフォルダには `<日時>.png` (例: `20261113-120000.png`) というファイルとして貼り付けられる。ファイルを優先して受け取るアプリでは、画像ではなく添付ファイルになることがある
- **貼り付けられる間:** 常駐が動いている間だけファイルとして貼り付けられる。常駐を終了した後と、常駐なしで実行した CLI (`--mode` / `--pipeline`) では、画像としての貼り付けだけができる。ClipRefiner を管理者として実行している場合も、エクスプローラへ貼り付けられないことがある
- **失敗したとき:** ファイルを載せられなくても、ビットマップの書き込みは成功として扱う

### 正規表現モード

正規表現モードは `[regex]` セクションの**有効プリセット**のステップを上から順に適用する。トレイで選んだ操作 (`regex-replace` / `regex-extract` / `regex-delete` / `regex-split`) が全ステップに同じ操作として適用される。パイプライン内の正規表現モードも同じ有効プリセットを参照する。

### 全角・半角変換の対象

対象は英数字・記号 (`!` 〜 `~` ↔ `！` 〜 `～`、`¥` ↔ `￥`)、スペース (半角スペース ↔ 全角スペース)、カタカナと約物 (`。` `「` `」` `、` `・` `ー` `゛` `゜`) である。半角へ変換すると濁音・半濁音は清音と `ﾞ` / `ﾟ` の 2 文字に分かれ、全角へ変換すると半角カナ直後の `ﾞ` / `ﾟ` を 1 文字に合成する。ひらがな・漢字や、半角に対応する文字が無いカタカナ (`ヵ` `ヶ` `ヰ` `ヱ` `ヮ`) は変換しない。

## 変換モードの使用例

<details>
<summary><strong>URL デコード</strong> (<code>url-decode</code>)</summary>
<br>

| | |
| :--- | :--- |
| **入力** | `https://example.com/search?q=%E3%83%86%E3%82%B9%E3%83%88&page=2` |
| **出力** | `https://example.com/search?q=テスト&page=2` |

</details>

<details>
<summary><strong>UTM パラメータの削除</strong> (<code>remove-utm</code>)</summary>
<br>

| | |
| :--- | :--- |
| **入力** | `https://example.com/page?id=123&utm_source=twitter&utm_medium=social` |
| **出力** | `https://example.com/page?id=123` |

削除するのは `utm_` で始まるパラメータと、広告のクリック ID (`fbclid` `gclid` `dclid` `gbraid` `wbraid` `msclkid` `yclid` `twclid` `ttclid` `igshid`)、および `igsh` `srsltid` `mc_eid` `gad_source` `_gl`。キーの大文字・小文字は区別しない (`UTM_Source` も削除する)。`%75tm_source` のようにパーセントエンコードされたキーもデコードして判定する。

フラグメント (`#` 以降) は残す。SPA のルーティングなどでフラグメントの中に `?` で始まるクエリがある場合は、そこからも削除する (`https://example.com/?id=1#/page?utm_source=x` → `https://example.com/?id=1#/page`)。

HTML ソースやメールからコピーした、区切りが `&amp;` にエスケープされた URL も扱える (`?utm_source=x&amp;id=1` → `?id=1`)。残すパラメータの間の `&amp;` はそのまま維持する。

URL を含む文章や URL の一覧も扱える。URL 以外の文字列はそのまま残す。`?` の前の単語に `/` が含まれるか、`www.` で始まるか、`example.com` / `example.com:8080` のようなホスト名 (`.` 区切りで、末尾が英字 2 文字以上) の場合だけ URL とみなすため、`What?utm_source=1` のような文章や `?utm_source=a` だけの文字列は変更しない。クエリの終わりは、空白 (改行を含む)・日本語などの ASCII 以外の文字・`)` `>` `<` `"` `'` `` ` `` のいずれかが現れた位置とし、末尾の句読点 (`.` `,` `;` `:` `!` `?`) は含めない。

</details>

<details>
<summary><strong>Base64 エンコード・デコード</strong> (<code>base64-encode</code> / <code>base64-decode</code>)</summary>
<br>

| モード | 入力 | 出力 |
| :--- | :--- | :--- |
| `base64-encode` | `ClipRefiner` | `Q2xpcFJlZmluZXI=` |
| `base64-decode` | `Q2xpcFJlZmluZXI=` | `ClipRefiner` |

デコードできない入力 (Base64 として不正、またはデコード結果が画像でも UTF-8 の文字列でもない) は変更しない。デコード結果が画像のときの扱いは [Base64 と画像](#base64-と画像) を参照。

</details>

<details>
<summary><strong>ファイルパス</strong> (<code>extract-basename</code> / <code>add-path-quotes</code>)</summary>
<br>

| モード | 入力 | 出力 |
| :--- | :--- | :--- |
| `extract-basename` | `C:\Users\sample\Documents\report.txt` | `report.txt` |
| `add-path-quotes` | `C:\Program Files\App\app.exe` | `"C:\Program Files\App\app.exe"` |

</details>

<details>
<summary><strong>行の並べ替え・重複削除</strong> (<code>sort-lines-asc</code> / <code>remove-duplicate-lines</code>)</summary>
<br>

| モード | 入力 | 出力 |
| :--- | :--- | :--- |
| `sort-lines-asc` | `banana` / `apple` / `cherry` | `apple` / `banana` / `cherry` |
| `sort-lines-asc` | `10` / `9` / `2` | `10` / `2` / `9` |
| `sort-lines-natural-asc` | `10` / `9` / `2` | `2` / `9` / `10` |
| `remove-duplicate-lines` | `a` / `b` / `a` / `c` / `b` | `a` / `b` / `c` |

(`/` は行の区切りを表す)

</details>

<details>
<summary><strong>空白・空行の除去</strong> (<code>trim-lines</code> / <code>remove-empty-lines</code>)</summary>
<br>

<!-- 入力例は前後の空白を見せるため、コード内の空白を許す -->
<!-- markdownlint-disable MD038 -->
| モード | 入力 | 出力 |
| :--- | :--- | :--- |
| `trim-lines` | `  foo  ` / `    bar   ` | `foo` / `bar` |
| `remove-empty-lines` | `a` / (空行) / `b` / (空行 2 行) / `c` | `a` / `b` / `c` |
<!-- markdownlint-enable MD038 -->

(`/` は行の区切りを表す)

</details>

<details>
<summary><strong>正規表現置換</strong> (<code>regex-replace</code>)</summary>
<br>

**設定 (`config.toml`):**

```toml
[regex]
active_preset_id = "default"
presets = [
  { id = "default", name = "デフォルト", steps = [
    { pattern = "(\\d{4})-(\\d{2})-(\\d{2})", replacement = "$1/$2/$3", case_insensitive = false, multiline = false },
  ]},
]
```

| | |
| :--- | :--- |
| **入力** | `会議日: 2024-01-15、締切: 2024-02-28` |
| **出力** | `会議日: 2024/01/15、締切: 2024/02/28` |

</details>

<details>
<summary><strong>JSON 整形</strong> (<code>json-format</code>)</summary>
<br>

**入力:**

```json
{"name":"ClipRefiner","tags":["a","b"],"nested":{"x":1}}
```

**出力 (キーを辞書順にソート):**

```json
{
  "name": "ClipRefiner",
  "nested": {
    "x": 1
  },
  "tags": [
    "a",
    "b"
  ]
}
```

先頭に BOM が付いた JSON も整形する。同じオブジェクトにキーが重複している JSON は、後の値で黙って上書きせず、整形しない (`json-format-preserve-order` も同じ)。

</details>

<details>
<summary><strong>YAML から JSON へ</strong> (<code>yaml-to-json</code>)</summary>
<br>

**入力:**

```yaml
name: ClipRefiner
items:
  - a
  - b
```

**出力:**

```json
{
  "name": "ClipRefiner",
  "items": [
    "a",
    "b"
  ]
}
```

マッピングも配列も含まない入力 (ただの文章・数値・コメントだけの行など) は変換しない。`plain text here` に引用符を付けて `"plain text here"` にすることはなく、自動変換に入れてもコピーした文章はそのまま残る。

`.inf` / `.nan` のように JSON で表現できない値を含む YAML は、`null` へ変えず変換しない。同じマッピングにキーが重複している YAML も、後の値で黙って上書きせず変換しない。

`---` で区切った複数のドキュメントは、ドキュメントを順に並べた配列にする。ドキュメントが 1 つだけなら配列にしない。どのドキュメントもマッピング・配列でない場合 (文章を `---` で区切っただけの場合など) は変換しない。

マージキー (`<<: *base`) は展開する。明示したキーを優先し、マージ元にだけあるキーはオブジェクトの末尾に加える。

`!Ref` や `!custom` のようなタグは捨てて、付けられた値だけを変換する (`a: !Ref b` は `{"a": "b"}`)。

先頭に BOM が付いた YAML も変換する。

数値は桁を保って変換する。128 bit 整数を超える整数 (`123456789012345678901234567890123456789012`) や 17 桁を超える小数 (`3.141592653589793238462643`) も丸めず、元の表記のまま JSON の数値にする。

</details>

<details>
<summary><strong>JSON から YAML へ</strong> (<code>json-to-yaml</code>)</summary>
<br>

数値は桁を保って変換する。128 bit 整数を超える整数や 17 桁を超える小数も丸めず、元の表記のまま YAML の数値にする。指数表記は YAML 1.1 のパーサーでも小数として読めるよう、`1e5` を `1.0e+5` の形に直す。

オブジェクトでも配列でもない JSON (`"plain text"` や `123` のような値だけの入力) は変換しない。引用符を外したり末尾に改行を足したりすることはない。

倍精度で表せない大きさの数値 (`1e400` など) を含む JSON や、同じオブジェクトにキーが重複している JSON は、変換しない。先頭に BOM が付いた JSON は変換する。

</details>

<details>
<summary><strong>Markdown から HTML へ</strong> (<code>markdown-to-html</code>)</summary>
<br>

**入力:**

```markdown
# 見出し

- 項目1
- 項目2
```

**出力 (HTML ソース):**

```html
<h1>見出し</h1>
<ul>
<li>項目1</li>
<li>項目2</li>
</ul>
```

</details>

<details>
<summary><strong>Web ページから Markdown へ</strong> (<code>html-to-markdown</code>)</summary>
<br>

ブラウザで見出しとリストを選択してコピーした場合 (HTML 形式):

```html
<h2>手順</h2>
<ol>
    <li><b>設定</b>を開く</li>
    <li><a href="https://example.com/">公式サイト</a>を参照する</li>
</ol>
```

**出力:**

```markdown
## 手順

1. **設定**を開く
2. [公式サイト](https://example.com/)を参照する
```

</details>

<details>
<summary><strong>Excel と Markdown の表</strong> (<code>excel-to-markdown</code> / <code>markdown-to-excel</code>)</summary>
<br>

**入力 (Excel からコピーした TSV):**

```text
ID	Name	Price
1	Apple	150
2	Banana	100
```

**`excel-to-markdown` の出力:**

```markdown
| ID | Name | Price |
|---|---|---|
| 1 | Apple | 150 |
| 2 | Banana | 100 |
```

`markdown-to-excel` はこの表を元の TSV へ戻す。セルの改行・`|` の扱いは [Excel と Markdown の表](#excel-と-markdown-の表) を参照。

</details>

<details>
<summary><strong>日時変換</strong> (<code>to-datetime-string</code>)</summary>
<br>

ローカルタイムゾーン基準。出力例はタイムゾーンにより異なる。旧名 `timestamp-to-datetime` / `datetime-to-timestamp` も CLI の `--mode` と `config.toml` (`TimestampToDatetime` / `DatetimeToTimestamp`) で使える。

| | |
| :--- | :--- |
| **入力** | `1700000000` |
| **出力例 (JST)** | `2023-11-15 07:13:20` |

- ミリ秒 (13 桁) は小数点以下 3 桁を付けて変換する (`1700000000123` → `2023-11-15 07:13:20.123`)。`to-unix-timestamp` は小数点以下の秒がある日時をミリ秒に戻す
- 出力形式ごとに専用のモードがある (入力 `1700000000`、JST の例)。`to-unix-timestamp` は下表のどの形式も受け付ける

  | モード | 出力 |
  | :--- | :--- |
  | `to-datetime-string-utc` | `2023-11-14 22:13:20` |
  | `to-iso8601` | `2023-11-15T07:13:20+09:00` |
  | `to-iso8601-utc` | `2023-11-14T22:13:20Z` |
  | `to-rfc2822` | `Wed, 15 Nov 2023 07:13:20 +0900` |
  | `to-japanese-datetime` | `2023年11月15日 07時13分20秒` |

- ミリ秒は `to-datetime-string-utc`・`to-iso8601`・`to-iso8601-utc` で小数点以下 3 桁を付けて出力する。`to-rfc2822` と `to-japanese-datetime` は小数点以下を表せないため切り捨てる
- `to-rfc2822` はローカルタイムゾーンの年が 0〜9999 の範囲外になる日時を表せないため、その行は変換せず元のまま残す
- 入力の形式は行ごとに自動で判定し、`to-datetime-string` などの出力モードはどの形式でも上表の出力形式へ変換する。`to-unix-timestamp` は Unix タイムスタンプ以外の形式をタイムスタンプへ変換する。判定する形式は次のとおり

  | 入力形式 | 例 |
  | :--- | :--- |
  | Unix タイムスタンプ (秒・ミリ秒・マイクロ秒・ナノ秒) | `1700000000` / `1700000000123` / `1700000000123456` / `1700000000123456789` / `1700000000.5` |
  | Excel のシリアル値 | `45245.0` / `45245.5` |
  | ISO 8601 系 | `2023-11-14T22:13:20Z` / `2023-11-15 07:13` / `2023/11/15` / `2023.11.15` |
  | 区切りなし | `20231115` (YYYYMMDD) / `20231115071320` (YYYYMMDDHHMMSS) |
  | RFC 2822 | `Wed, 15 Nov 2023 07:13:20 +0900` |
  | 和文形式 | `2023年11月15日 07時13分20秒` / `2023年11月15日` / `2023年11月15日(水)` |
  | 和暦 | `令和5年11月15日` / `令和元年5月1日` / `R5.11.15 07:13` |
  | 月名・米国式 | `Nov 15, 2023 10:13 PM` / `15 November 2023` / `11/15/2023` (月/日/年) |

- 数値の桁数で単位を判定する (10 桁まで: 秒、11〜13 桁: ミリ秒、14〜16 桁: マイクロ秒、17 桁以上: ナノ秒)。ミリ秒より細かい桁は切り捨てる
- 扱える日時は西暦 -9999 年〜9999 年の範囲。範囲外になる Unix タイムスタンプの行は変換せず、元のまま残す
- 整数部が 61〜2958465 で小数部を持つ正の数は Excel のシリアル値 (1900-03-01〜9999-12-31) として扱い、小数部を時刻にする。`2024` や `45245` のような整数だけの値はシリアル値にせず、Unix タイムスタンプとして扱う (複数行テキスト中の年や連番が 1900 年代の日時に化けないようにするため)
- タイムゾーンのない入力 (Excel のシリアル値・和文形式・和暦・月名・米国式など) はローカルタイムゾーンとして扱い、日付のみの入力は 0 時として扱う。和暦の元号名 (`令和5年…`) は元号の改元日による範囲を検証しない。略号 (`R5.11.15`) はバージョン番号などとの誤検知を避けるため、月日を 2 桁で書き、元号の期間内の日付に限る。米国式は月/日/年のみで、日/月/年 (`15/11/2023`) は変換しない
- 全角の数字・記号 (`２０２３／１１／１５`・`：`・全角スペース) は半角として扱い、日付に続く曜日 (`(水)`・`(水曜日)`) は無視する
- 8 桁・14 桁の数字は、実在する日時 (年が 1000〜9999) のときだけ `YYYYMMDD`・`YYYYMMDDHHMMSS` として扱い、それ以外は Unix タイムスタンプとして扱う
- 複数行は行ごとに変換し、変換できない行はそのまま残す

</details>

<details>
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

| | |
| :--- | :--- |
| **入力** | `1234567` |
| **出力** | `1,234,567` |

</details>

<details>
<summary><strong>ケース変換</strong> (<code>to-snake-case</code>)</summary>
<br>

| | |
| :--- | :--- |
| **入力** | `HelloWorld` |
| **出力** | `hello_world` |

</details>

<details>
<summary><strong>全角・半角変換</strong> (<code>to-halfwidth</code>)</summary>
<br>

| | |
| :--- | :--- |
| **入力** | `ＡＢＣ　１２３　データ` |
| **出力** | `ABC 123 ﾃﾞｰﾀ` |

</details>
