//! Compare independent observations against the actual replay boundaries.
use crate::{
    decision::Observation,
    model::{BattleResult, Outcome, Termination, UnitResult},
};
use serde::{
    de::{
        value::{MapAccessDeserializer, StringDeserializer},
        MapAccess, Visitor,
    },
    Deserialize, Deserializer,
};
use std::{
    collections::BTreeSet,
    fmt::{self, Debug},
    marker::PhantomData,
};

/// An omitted field leaves the value unchecked; an explicit null is invalid.
pub(super) fn provided<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Require named fields without converting the map into a duplicate-dropping value.
#[derive(Debug)]
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object with named fields")
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }

        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

pub(super) fn provided_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Object::deserialize(deserializer).map(|Object(value)| Some(value))
}

fn unit_expectations<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UnitExpectation>, D::Error> {
    let objects = Vec::<Object<UnitExpectation>>::deserialize(deserializer)?;
    Ok(objects.into_iter().map(|Object(unit)| unit).collect())
}

fn provided_enum<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    let name = String::deserialize(deserializer)?;
    T::deserialize(StringDeserializer::<D::Error>::new(name)).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NumberExpectation {
    Exact(f64),
    Approximate(Object<Approximation>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Approximation {
    value: f64,
    abs_tol: f64,
}

fn check_number(
    expected: &Option<NumberExpectation>,
    actual: f64,
    field: &str,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let (value, tolerance) = match expected {
        NumberExpectation::Exact(value) => (*value, 0.0),
        NumberExpectation::Approximate(Object(approx)) => (approx.value, approx.abs_tol),
    };
    if !value.is_finite() || !tolerance.is_finite() || tolerance < 0.0 {
        return Err(format!(
            "{field}: expected a finite value and nonnegative finite abs_tol, got {value:?} and {tolerance:?}"
        ));
    }
    if !actual.is_finite() || (actual - value).abs() > tolerance {
        return Err(format!(
            "{field} mismatch: expected {value:?} (abs_tol {tolerance:?}), got {actual:?}"
        ));
    }
    Ok(())
}

fn check_value<T: Debug + PartialEq>(
    expected: Option<T>,
    actual: T,
    field: &str,
) -> Result<(), String> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(format!(
                "{field} mismatch: expected {expected:?}, got {actual:?}"
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StateExpectation {
    #[serde(default, deserialize_with = "provided")]
    wave: Option<usize>,
    #[serde(default, deserialize_with = "provided")]
    cycle: Option<usize>,
    #[serde(default, deserialize_with = "provided")]
    elapsed_av: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    cycle_remaining_av: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    remaining_av: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    ally_skill_points: Option<u8>,
    #[serde(default, deserialize_with = "provided")]
    enemy_skill_points: Option<u8>,
    #[serde(default, deserialize_with = "unit_expectations")]
    units: Vec<UnitExpectation>,
}

struct ObservedState<'a> {
    cycle: usize,
    elapsed_av: f64,
    cycle_remaining_av: f64,
    remaining_av: f64,
    ally_skill_points: u8,
    enemy_skill_points: u8,
    units: &'a [UnitResult],
}

impl StateExpectation {
    pub(super) fn verify_observation(
        &self,
        wave: usize,
        observation: &Observation,
    ) -> Result<(), String> {
        self.verify(
            wave,
            ObservedState {
                cycle: observation.cycle,
                elapsed_av: observation.now,
                cycle_remaining_av: observation.cycle_remaining_av,
                remaining_av: observation.remaining_av,
                ally_skill_points: observation.ally_skill_points,
                enemy_skill_points: observation.enemy_skill_points,
                units: &observation.units,
            },
        )
    }

    pub(super) fn verify_result(&self, wave: usize, result: &BattleResult) -> Result<(), String> {
        self.verify(
            wave,
            ObservedState {
                cycle: result.cycle,
                elapsed_av: result.elapsed_av,
                cycle_remaining_av: result.cycle_remaining_av,
                remaining_av: result.remaining_av,
                ally_skill_points: result.ally_skill_points,
                enemy_skill_points: result.enemy_skill_points,
                units: &result.units,
            },
        )
    }

    fn verify(&self, wave: usize, actual: ObservedState<'_>) -> Result<(), String> {
        check_value(self.wave, wave, "wave")?;
        check_value(self.cycle, actual.cycle, "cycle")?;
        check_number(&self.elapsed_av, actual.elapsed_av, "elapsed_av")?;
        check_number(
            &self.cycle_remaining_av,
            actual.cycle_remaining_av,
            "cycle_remaining_av",
        )?;
        check_number(&self.remaining_av, actual.remaining_av, "remaining_av")?;
        check_value(
            self.ally_skill_points,
            actual.ally_skill_points,
            "ally_skill_points",
        )?;
        check_value(
            self.enemy_skill_points,
            actual.enemy_skill_points,
            "enemy_skill_points",
        )?;

        let mut requested = BTreeSet::new();
        for expected in &self.units {
            if !requested.insert(expected.id.as_str()) {
                return Err(format!("duplicate expected unit ID {:?}", expected.id));
            }
            let mut matches = actual.units.iter().filter(|unit| unit.id == expected.id);
            let actual = matches
                .next()
                .ok_or_else(|| format!("unit {:?} is not in this snapshot", expected.id))?;
            if matches.next().is_some() {
                return Err(format!(
                    "unit {:?} is ambiguous in this snapshot",
                    expected.id
                ));
            }
            expected
                .verify(actual)
                .map_err(|error| format!("unit {:?}.{error}", expected.id))?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitExpectation {
    id: String,
    #[serde(default, deserialize_with = "provided")]
    hp: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    shield: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    energy: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    toughness: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    alive: Option<bool>,
    #[serde(default, deserialize_with = "provided")]
    revivals_left: Option<u8>,
}

impl UnitExpectation {
    fn verify(&self, actual: &UnitResult) -> Result<(), String> {
        check_number(&self.hp, actual.hp, "hp")?;
        check_number(&self.shield, actual.shield, "shield")?;
        check_number(&self.energy, actual.energy, "energy")?;
        check_number(&self.toughness, actual.toughness, "toughness")?;
        check_value(self.alive, actual.alive, "alive")?;
        check_value(self.revivals_left, actual.revivals_left, "revivals_left")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResultExpectation {
    #[serde(default, deserialize_with = "provided_enum")]
    outcome: Option<Outcome>,
    #[serde(default, deserialize_with = "provided_enum")]
    termination: Option<Termination>,
    #[serde(default, deserialize_with = "provided")]
    penalty_av: Option<NumberExpectation>,
    #[serde(default, deserialize_with = "provided")]
    actions: Option<u32>,
    #[serde(default, deserialize_with = "provided")]
    waves_cleared: Option<usize>,
    #[serde(default, deserialize_with = "provided_object")]
    state: Option<StateExpectation>,
}

impl ResultExpectation {
    pub(super) fn verify(&self, wave: usize, result: &BattleResult) -> Result<(), String> {
        check_value(self.outcome, result.outcome, "outcome")?;
        check_value(self.termination, result.termination, "termination")?;
        check_number(&self.penalty_av, result.penalty_av, "penalty_av")?;
        check_value(self.actions, result.actions, "actions")?;
        check_value(self.waves_cleared, result.waves_cleared, "waves_cleared")?;
        if let Some(state) = &self.state {
            state
                .verify_result(wave, result)
                .map_err(|error| format!("state: {error}"))?;
        }
        Ok(())
    }
}
