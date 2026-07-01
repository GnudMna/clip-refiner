use toml_edit::DocumentMut;

/// レガシー `[[clips]]` を `config.toml` から除去する
pub(crate) fn apply(doc: &mut DocumentMut) {
    doc.as_table_mut().remove("clips");
}
