//! MCP クライアント (特に Claude Desktop) は bool や配列を JSON 文字列で
//! 送ることがあるため、両形式を受け付けるパラメータ型を提供する。
#![allow(dead_code)] // TODO: 全ツール実装後に削除

use schemars::JsonSchema;
use serde::Deserialize;

/// bool または "true"/"false" 文字列。
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BoolOrString {
    Bool(bool),
    Str(String),
}

impl BoolOrString {
    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            BoolOrString::Bool(b) => Ok(*b),
            BoolOrString::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(format!("Invalid boolean value: {other:?}")),
            },
        }
    }
}

/// Option<BoolOrString> をデフォルト付きで bool に解決する。
pub fn opt_bool(v: &Option<BoolOrString>, default: bool) -> Result<bool, String> {
    v.as_ref().map_or(Ok(default), BoolOrString::as_bool)
}

/// JSON 配列、または JSON 文字列化された配列 (例: "[100, 200]")。
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOrString<T> {
    List(Vec<T>),
    Str(String),
}

impl<T: serde::de::DeserializeOwned> ListOrString<T> {
    pub fn into_list(self) -> Result<Vec<T>, String> {
        match self {
            ListOrString::List(v) => Ok(v),
            ListOrString::Str(s) => {
                serde_json::from_str(&s).map_err(|e| format!("Invalid list value {s:?}: {e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_variants() {
        assert!(BoolOrString::Bool(true).as_bool().unwrap());
        assert!(BoolOrString::Str("True".into()).as_bool().unwrap());
        assert!(!BoolOrString::Str("false".into()).as_bool().unwrap());
        assert!(BoolOrString::Str("yes".into()).as_bool().is_err());
        assert!(opt_bool(&None, true).unwrap());
    }

    #[test]
    fn list_variants() {
        let l: ListOrString<i32> = serde_json::from_str("[1, 2]").unwrap();
        assert_eq!(l.into_list().unwrap(), vec![1, 2]);
        let s: ListOrString<i32> = serde_json::from_str(r#""[3, 4]""#).unwrap();
        assert_eq!(s.into_list().unwrap(), vec![3, 4]);
        let bad: ListOrString<i32> = serde_json::from_str(r#""oops""#).unwrap();
        assert!(bad.into_list().is_err());
    }
}
