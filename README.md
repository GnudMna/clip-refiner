# ClipRefiner

```
クリップボードのテキストを加工するツール

使用方法:
    引数なし: システムトレイに常駐し、クリップボードを監視して自動加工
    --mode指定: クリップボードの内容を一度だけ加工

Options:
  -m, --mode <MODE>
          実行モードの指定

          Possible values:
          - url-encode:                  URLエンコード
          - url-decode:                  URLデコード
          - remove-utm:                  UTMパラメータを削除
          - trim:                        改行や空白を整形
          - trim-lines:                  行単位で改行や空白を整形
          - json-format:                 JSON形式を整形(キー順序不同)
          - json-format-preserve-order:  JSON形式を整形(キー順序保持)
          - json-to-yaml:                JSON形式をYAML形式へ変換(キー順序不同)
          - json-to-yaml-preserve-order: JSON形式をYAML形式へ変換(キー順序保持)
          - yaml-to-json:                YAML形式をJSON形式へ変換(キー順序不同)
          - yaml-to-json-preserve-order: YAML形式をJSON形式へ変換(キー順序保持)
          - add-comma:                   カンマ無し数値をカンマ区切りの数値に
          - remove-comma:                カンマ区切りの数値をカンマ無し数値に
          - sort-lines:                  行単位で並び替え

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
