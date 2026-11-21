# 変換モード

変換モードの一覧と使用例をまとめる。監視で適用するモードの指定は [設定リファレンス](configuration.md#設定項目)、複数モードの連鎖は [加工パイプライン](configuration.md#加工パイプライン-pipeline) を参照。

## 変換モード一覧

`--mode` に渡す値 (CLI 名) と、トレイメニュー・クイックセレクターでの表示。**全 46 モード**に対応。

> **設定ファイル:** CLI は kebab-case (`url-decode` など) だが、`config.toml` では PascalCase (`UrlDecode` など) を指定する。初回保存時に生成される `config.toml` を参照するとよい。

| カテゴリ               | モード名 (`--mode`)                                                                                | 説明                                                                 |
| :--------------------- | :------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------- |
| **URL 操作**           | `url-encode` / `url-decode`                                                                        | URL のエンコード・デコード                                           |
|                        | `remove-utm`                                                                                       | URL から `utm_*` 計測パラメータを削除                                |
| **ファイルパス**       | `extract-basename` / `extract-basename-quoted`                                                     | パスからファイル名のみを抽出 (引用符付きオプションあり)              |
|                        | `add-path-quotes` / `remove-path-quotes`                                                           | パスへの引用符 (`"`) の付与・削除                                    |
|                        | `path-to-slash` / `path-to-backslash`                                                              | パス区切り文字をスラッシュ / バックスラッシュに変換                  |
| **行の並べ替え・削除** | `sort-lines-asc` / `sort-lines-desc`                                                               | 行単位での昇順・降順ソート (CSV 対応)                                |
|                        | `remove-empty-lines`                                                                               | 空行を削除                                                           |
|                        | `remove-duplicate-lines`                                                                           | 重複行を削除                                                         |
| **空白の除去**         | `trim`                                                                                             | テキスト全体の前後の空白・改行を削除                                 |
|                        | `trim-lines`                                                                                       | 行ごとに前後の空白を削除                                             |
| **エスケープ・解除**   | `escape` / `unescape`                                                                              | バックスラッシュエスケープの付与・解除                               |
|                        | `regex-escape` / `regex-unescape`                                                                  | 正規表現メタ文字のエスケープ・解除                                   |
| **正規表現**           | `regex-replace`                                                                                    | パターンに一致する部分を置換 (`[regex]` 設定を使用)                  |
|                        | `regex-extract`                                                                                    | パターンに一致する部分を行単位で抽出                                 |
|                        | `regex-delete`                                                                                     | パターンに一致する部分を削除                                         |
|                        | `regex-split`                                                                                      | パターンで分割し改行で結合                                           |
| **JSON 整形**          | `json-format`                                                                                      | JSON をインデント整形 (キーを辞書順にソート)                         |
|                        | `json-format-preserve-order`                                                                       | JSON をインデント整形 (キー順序保持)                                 |
| **YAML→JSON**          | `yaml-to-json`                                                                                     | YAML を JSON へ変換 (キー順序不定)                                   |
|                        | `yaml-to-json-preserve-order`                                                                      | YAML を JSON へ変換 (キー順序保持)                                   |
| **JSON→YAML**          | `json-to-yaml`                                                                                     | JSON を YAML へ変換 (キーを辞書順にソート)                           |
|                        | `json-to-yaml-preserve-order`                                                                      | JSON を YAML へ変換 (キー順序保持)                                   |
| **Markdown・HTML**     | `markdown-to-html`                                                                                 | Markdown を HTML ソース (テキスト) へ変換                            |
|                        | `markdown-to-rich-text`                                                                            | Markdown をリッチテキスト (HTML 形式) としてクリップボードへ書き込む |
|                        | `html-to-markdown`                                                                                 | HTML を Markdown へ変換 (ブラウザでコピーした HTML 形式を優先)       |
| **Excel**              | `excel-to-markdown`                                                                                | Excel コピーデータを Markdown テーブルへ変換                         |
|                        | `markdown-to-excel`                                                                                | Markdown 表を Excel (TSV) 形式へ変換                                 |
|                        | `excel-to-image`                                                                                   | Excel コピーデータの見た目を画像としてクリップボードへ保存           |
| **Unixタイムスタンプ** | `timestamp-to-datetime` / `datetime-to-timestamp`                                                  | Unix タイムスタンプ ↔ 日時文字列の変換 (ローカルタイムゾーン)        |
| **桁区切りカンマ**     | `add-comma` / `remove-comma`                                                                       | 数値への 3 桁カンマ区切り付与・削除                                  |
| **識別子の書式**       | `to-camel-case` / `to-snake-case` / `to-pascal-case` / `to-kebab-case` / `to-screaming-snake-case` | 識別子のケース変換                                                   |
| **全角・半角**         | `to-halfwidth` / `to-fullwidth`                                                                    | 英数字・記号・スペース・カタカナの全角 ↔ 半角変換                    |

全角・半角変換の対象は英数字・記号 (`!` 〜 `~` ↔ `！` 〜 `～`)、スペース (半角スペース ↔ 全角スペース)、カタカナと約物 (`。` `「` `」` `、` `・` `ー` `゛` `゜`) である。半角へ変換すると濁音・半濁音は清音と `ﾞ` / `ﾟ` の 2 文字に分かれ、全角へ変換すると半角カナ直後の `ﾞ` / `ﾟ` を 1 文字に合成する。ひらがな・漢字や、半角に対応する文字が無いカタカナ (`ヵ` `ヶ` `ヰ` `ヱ` `ヮ`) は変換しない。

### リッチテキストと HTML 形式

`markdown-to-rich-text` と `html-to-markdown` はテキストだけでなく、クリップボードの HTML 形式 (`CF_HTML`) を読み書きする。

- **`markdown-to-rich-text`:** Markdown を HTML へ変換し、HTML 形式としてクリップボードへ書き込む。Word / Outlook / Teams などに貼り付けると、見出し・リスト・表が整形済みで入る。表には罫線を付ける。プレーンテキスト側には変換前の Markdown を残すため、テキストエディタに貼り付けると Markdown のままになる。HTML ソースをテキストとして得たい場合は `markdown-to-html` を使う
- **`html-to-markdown`:** ブラウザや Word からコピーした内容に HTML 形式があれば、テキストより HTML 形式を優先して Markdown へ変換し、テキストとして書き戻す (HTML 形式は消える)。HTML 形式が無い場合はテキストを HTML ソースとして変換する。タグを含まないテキストは変換しない。見出し・段落・強調・リンク・画像・リスト・引用・コードブロック・表に対応し、`script` / `style` などの非表示要素は出力しない。要素の入れ子が 256 階層を超える HTML は変換しない
- **パイプライン:** `html-to-markdown` は先頭に置いたときだけ HTML 形式を読み、`markdown-to-rich-text` はテキスト加工の最終段に置いたときだけ HTML 形式を書き込む。それ以外の位置ではテキスト同士の変換として働く (`markdown-to-rich-text` は `markdown-to-html` と同じ HTML ソースを返す)。例: `--pipeline excel-to-markdown markdown-to-rich-text` で Excel の表を罫線付きの表として貼り付けられる

正規表現モードは `[regex]` セクションの**有効プリセット**のステップを上から順に適用する。トレイで選んだ操作 (`regex-replace` / `extract` / `delete` / `split`) が全ステップに同じ操作として適用される。パイプライン内の正規表現モードも同じ有効プリセットを参照する。

## 変換モードの使用例

<details>
<summary><strong>UTM パラメータの削除</strong> (<code>remove-utm</code>)</summary>
<br>

|          |                                                                        |
| :------- | :--------------------------------------------------------------------- |
| **入力** | `https://example.com/page?id=123&utm_source=twitter&utm_medium=social` |
| **出力** | `https://example.com/page?id=123`                                      |

</details>

<details>
<summary><strong>Excel から Markdown へ</strong> (<code>excel-to-markdown</code>)</summary>
<br>

**入力 (TSV):**

```
ID	Name	Price
1	Apple	150
2	Banana	100
```

**出力:**

```markdown
| ID  | Name   | Price |
| --- | ------ | ----- |
| 1   | Apple  | 150   |
| 2   | Banana | 100   |
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

|          |                                        |
| :------- | :------------------------------------- |
| **入力** | `会議日: 2024-01-15、締切: 2024-02-28` |
| **出力** | `会議日: 2024/01/15、締切: 2024/02/28` |

</details>

<details>
<summary><strong>タイムスタンプ変換</strong> (<code>timestamp-to-datetime</code>)</summary>
<br>

ローカルタイムゾーン基準。出力例はタイムゾーンにより異なる。

|                  |                       |
| :--------------- | :-------------------- |
| **入力**         | `1700000000`          |
| **出力例 (JST)** | `2023-11-15 07:13:20` |

</details>

<details>
<summary><strong>ケース変換</strong> (<code>to-snake-case</code>)</summary>
<br>

|          |               |
| :------- | :------------ |
| **入力** | `HelloWorld`  |
| **出力** | `hello_world` |

</details>

<details>
<summary><strong>全角・半角変換</strong> (<code>to-halfwidth</code>)</summary>
<br>

|          |                          |
| :------- | :----------------------- |
| **入力** | `ＡＢＣ　１２３　データ` |
| **出力** | `ABC 123 ﾃﾞｰﾀ`           |

</details>

<details>
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

|          |             |
| :------- | :---------- |
| **入力** | `1234567`   |
| **出力** | `1,234,567` |

</details>
