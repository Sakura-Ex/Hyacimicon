//! Reads the diagnostic file and settlement objects without losing duplicate keys.
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

/// Parses the file envelope and settlement data before they enter a JSON map.
pub(super) fn parse(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = Object::Scenario.deserialize(&mut parser)?;
    parser.end()?;
    Ok(value)
}

/// Object paths whose fields participate in the diagnostic import contract.
#[derive(Clone, Copy)]
enum Object {
    Scenario,
    Mechanics,
    Settlement,
    EnemyWeights,
}

impl Object {
    fn path(self) -> &'static str {
        match self {
            Self::Scenario => "$",
            Self::Mechanics => "$.mechanics",
            Self::Settlement => "$.mechanics.settlement",
            Self::EnemyWeights => "$.mechanics.settlement.enemy_weights",
        }
    }
}

impl<'de> DeserializeSeed<'de> for Object {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, parser: D) -> Result<Value, D::Error> {
        parser.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Object {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an object at {}", self.path())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Value, A::Error> {
        let mut fields = Map::new();
        while let Some(key) = input.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(A::Error::custom(format!(
                    "duplicate field {key:?} at {}",
                    self.path()
                )));
            }
            let value = match (self, key.as_str()) {
                (Self::Scenario, "mechanics") => input.next_value_seed(Self::Mechanics)?,
                (Self::Mechanics, "settlement") => input.next_value_seed(Self::Settlement)?,
                (Self::Settlement, "enemy_weights") => input.next_value_seed(Self::EnemyWeights)?,
                _ => input.next_value()?,
            };
            fields.insert(key, value);
        }
        Ok(Value::Object(fields))
    }
}
