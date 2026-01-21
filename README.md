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
          - url-encode:   URLエンコード
          - url-decode:   URLデコード
          - remove-utm:   UTMパラメータを削除
          - trim:         改行や空白を整形する
          - json-format:  JSON形式を整形する
          - add-comma:    数値をカンマ区切りにする
          - remove-comma: カンマ区切りを数値にする
          - sort-lines:   行単位で並び替える

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
