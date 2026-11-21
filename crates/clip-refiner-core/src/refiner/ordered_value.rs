use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ======================================================================
// 順序保持値型
// ======================================================================
/// JSONやYAMLのパース時にキーの順序を保持するための値構造
///
/// `serde_json::Value` と似ているが、オブジェクトの保持に `IndexMap` を使用し、
/// データの順序を維持したままシリアライズ・デシリアライズが可能
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrderedValue {
    /// JSON の null 値
    Null,
    /// 真偽値
    Bool(bool),
    /// 数値
    Number(serde_json::Number),
    /// 文字列
    String(String),
    /// 配列
    Array(Vec<OrderedValue>),
    /// キー順序を保持するオブジェクト
    Object(IndexMap<String, OrderedValue>),
}
