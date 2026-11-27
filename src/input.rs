//! Bounded file reading and strict JSON parsing.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::error::Error;

/// Default maximum size of one input file: 64 MiB.
pub const DEFAULT_MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest value accepted for `--max-input-bytes`: 512 MiB.
pub const HARD_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Reads a regular file, refusing anything larger than `limit` bytes.
///
/// Directories, devices, pipes and other non-regular files are rejected
/// before they are opened, and again on the opened handle. The read itself
/// is capped, so a file that grows while being read cannot exceed the limit.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let metadata = std::fs::metadata(path).map_err(|e| Error::new(format!("cannot read: {e}")))?;
    if !metadata.is_file() {
        return Err(Error::new("not a regular file"));
    }
    if metadata.len() > limit {
        return Err(too_large(limit));
    }

    let file = File::open(path).map_err(|e| Error::new(format!("cannot open: {e}")))?;
    let opened = file
        .metadata()
        .map_err(|e| Error::new(format!("cannot read: {e}")))?;
    if !opened.is_file() {
        return Err(Error::new("not a regular file"));
    }

    let capacity = usize::try_from(opened.len().min(limit)).unwrap_or(0);
    let mut buffer = Vec::with_capacity(capacity);
    file.take(limit.saturating_add(1))
        .read_to_end(&mut buffer)
        .map_err(|e| Error::new(format!("cannot read: {e}")))?;
    if buffer.len() as u64 > limit {
        return Err(too_large(limit));
    }
    Ok(buffer)
}

fn too_large(limit: u64) -> Error {
    Error::new(format!(
        "file is larger than the input limit of {limit} bytes (see --max-input-bytes)"
    ))
}

/// Parses one JSON document strictly.
///
/// On top of normal JSON syntax rules this rejects empty input, trailing
/// data after the document and objects that repeat a key. Repeated keys are
/// rejected because JSON parsers disagree on which value wins, so two tools
/// could read the same SBOM differently. A leading UTF-8 byte order mark is
/// skipped. Nesting deeper than 128 levels is rejected by `serde_json`.
pub fn parse_strict_json(bytes: &[u8]) -> Result<Value, Error> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::new("invalid JSON: the file is empty"));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let StrictValue(value) = StrictValue::deserialize(&mut deserializer)
        .map_err(|e| Error::new(format!("invalid JSON: {e}")))?;
    deserializer
        .end()
        .map_err(|e| Error::new(format!("invalid JSON: {e}")))?;
    Ok(value)
}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(StrictValue)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number is not finite"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(StrictValue(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                // The key itself is not echoed: it is untrusted input.
                return Err(de::Error::custom("an object repeats the same key"));
            }
            let StrictValue(value) = map.next_value()?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_keys() {
        let err = parse_strict_json(br#"{"a": 1, "a": 2}"#).unwrap_err();
        assert!(err.to_string().contains("repeats the same key"), "{err}");
    }

    #[test]
    fn rejects_nested_duplicate_keys() {
        let err = parse_strict_json(br#"{"x": [{"name": "a", "name": "b"}]}"#).unwrap_err();
        assert!(err.to_string().contains("repeats the same key"));
    }

    #[test]
    fn rejects_trailing_data() {
        assert!(parse_strict_json(br#"{"a": 1} {"b": 2}"#).is_err());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(parse_strict_json(b"  \n").is_err());
    }

    #[test]
    fn accepts_byte_order_mark() {
        let value = parse_strict_json(b"\xEF\xBB\xBF{\"a\": 1}").unwrap();
        assert_eq!(value["a"], 1);
    }

    #[test]
    fn rejects_excessive_nesting() {
        let doc = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(parse_strict_json(doc.as_bytes()).is_err());
    }

    #[test]
    fn error_does_not_echo_content() {
        let err = parse_strict_json(br#"{"secret-token-value": 1, "secret-token-value": 2}"#)
            .unwrap_err()
            .to_string();
        assert!(!err.contains("secret-token-value"));
    }
}
