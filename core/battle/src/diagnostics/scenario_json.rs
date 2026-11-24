//! Preserve scenario values for fingerprinting while rejecting duplicate keys.

use serde::{
    de::{Error, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Map, Value};
use std::fmt;

pub(super) struct UniqueValue(pub(super) Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor).map(Self)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut input: M) -> Result<Value, M::Error> {
        let mut result = Map::new();
        while let Some(key) = input.next_key::<String>()? {
            if result.contains_key(&key) {
                return Err(M::Error::custom(format_args!(
                    "duplicate scenario key {key:?}"
                )));
            }
            result.insert(key, input.next_value::<UniqueValue>()?.0);
        }
        Ok(Value::Object(result))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut input: S) -> Result<Value, S::Error> {
        let mut result = Vec::new();
        while let Some(value) = input.next_element::<UniqueValue>()? {
            result.push(value.0);
        }
        Ok(Value::Array(result))
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}
