# 更新署名の公開鍵

アプリ更新の MSI / ポータブル ZIP を検証する minisign 公開鍵 (`*.pub`) を置く。`crates/clip-refiner-updater/build.rs` がこのディレクトリの `*.pub` をすべてアプリへ埋め込み、いずれかの鍵で検証できた署名だけを受け入れる。

秘密鍵はここに置かない。生成・入れ替えの手順は [リリースの更新署名鍵](../../docs/dev/release.md#更新署名鍵) を参照。
