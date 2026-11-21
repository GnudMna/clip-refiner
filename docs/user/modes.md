# 変換モード

変換モードの一覧と使用例をまとめる。監視で適用するモードの指定は [設定リファレンス](configuration.md#設定項目)、複数モードの連鎖は [加工パイプライン](configuration.md#加工パイプライン-pipeline) を参照。

## 変換モード一覧

`--mode` に渡す値 (CLI 名) と、トレイメニュー・クイックセレクターでの表示。**全 42 モード**に対応。

> **設定ファイル:** CLI は kebab-case (`url-decode` など) だが、`config.toml` では PascalCase (`UrlDecode` など) を指定する。初回保存時に生成される `config.toml` を参照するとよい。

| カテゴリ        | モード名 (`--mode`)                                                                                | 説明                                                          |
| :-------------- | :------------------------------------------------------------------------------------------------- | :------------------------------------------------------------ |
| **URL 操作**    | `url-encode` / `url-decode`                                                                        | URL のエンコード・デコード                                    |
|                 | `remove-utm`                                                                                       | URL から `utm_*` 計測パラメータを削除                         |
| **パス操作**    | `extract-basename` / `extract-basename-quoted`                                                     | パスからファイル名のみを抽出 (引用符付きオプションあり)       |
|                 | `add-path-quotes` / `remove-path-quotes`                                                           | パスへの引用符 (`"`) の付与・削除                             |
|                 | `path-to-slash` / `path-to-backslash`                                                              | パス区切り文字をスラッシュ / バックスラッシュに変換           |
| **行操作**      | `sort-lines-asc` / `sort-lines-desc`                                                               | 行単位での昇順・降順ソート (CSV 対応)                         |
|                 | `remove-empty-lines`                                                                               | 空行を削除                                                    |
|                 | `remove-duplicate-lines`                                                                           | 重複行を削除                                                  |
| **トリム**      | `trim`                                                                                             | テキスト全体の前後の空白・改行を削除                          |
|                 | `trim-lines`                                                                                       | 行ごとに前後の空白を削除                                      |
| **エスケープ**  | `escape` / `unescape`                                                                              | バックスラッシュエスケープの付与・解除                        |
|                 | `regex-escape` / `regex-unescape`                                                                  | 正規表現メタ文字のエスケープ・解除                            |
| **正規表現**    | `regex-replace`                                                                                    | パターンに一致する部分を置換 (`[regex]` 設定を使用)           |
|                 | `regex-extract`                                                                                    | パターンに一致する部分を行単位で抽出                          |
|                 | `regex-delete`                                                                                     | パターンに一致する部分を削除                                  |
|                 | `regex-split`                                                                                      | パターンで分割し改行で結合                                    |
| **JSON 整形**   | `json-format`                                                                                      | JSON をインデント整形 (キー順序不定)                          |
|                 | `json-format-preserve-order`                                                                       | JSON をインデント整形 (キー順序保持)                          |
| **JSON へ変換** | `yaml-to-json`                                                                                     | YAML を JSON へ変換 (キー順序不定)                            |
|                 | `yaml-to-json-preserve-order`                                                                      | YAML を JSON へ変換 (キー順序保持)                            |
| **YAML へ変換** | `json-to-yaml`                                                                                     | JSON を YAML へ変換 (キー順序不定)                            |
|                 | `json-to-yaml-preserve-order`                                                                      | JSON を YAML へ変換 (キー順序保持)                            |
| **Markdown**    | `markdown-to-html`                                                                                 | Markdown を HTML へ変換                                       |
| **Excel**       | `excel-to-markdown`                                                                                | Excel コピーデータを Markdown テーブルへ変換                  |
|                 | `markdown-to-excel`                                                                                | Markdown 表を Excel (TSV) 形式へ変換                          |
|                 | `excel-to-image`                                                                                   | Excel コピーデータの見た目を画像としてクリップボードへ保存    |
| **日時変換**    | `timestamp-to-datetime` / `datetime-to-timestamp`                                                  | Unix タイムスタンプ ↔ 日時文字列の変換 (ローカルタイムゾーン) |
| **数値変換**    | `add-comma` / `remove-comma`                                                                       | 数値への 3 桁カンマ区切り付与・削除                           |
| **ケース変換**  | `to-camel-case` / `to-snake-case` / `to-pascal-case` / `to-kebab-case` / `to-screaming-snake-case` | 識別子のケース変換                                            |

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
<summary><strong>カンマ区切り付与</strong> (<code>add-comma</code>)</summary>
<br>

|          |             |
| :------- | :---------- |
| **入力** | `1234567`   |
| **出力** | `1,234,567` |

</details>
