//! 按指定资源条件生成可直接交给 account-core 的原始账号。

#![forbid(unsafe_code)]

mod allocation;
mod reference;
mod relics;

use std::{collections::BTreeMap, fmt};

use account_core::{
    CharacterConfig, LightConeInstance, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount, TraceConfig,
};
pub use rand_chacha::ChaCha8Rng;
pub use rand_chacha::rand_core::{RngCore, SeedableRng};
pub use reference::{AccountReference, ReferenceOverrides};
pub use relics::{
    RelicRequest, RelicSpec, generate_character_relics, generate_relic,
    maximum_character_relic_score, score_relic,
};
use serde::{Deserialize, Serialize};

pub const GENERATOR_VERSION: &str = "account-rust-v3";

/// 五星角色和光锥的投入类别，计数单位为一份本体、星魂或叠影。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldCategory {
    Limited,
    Standard,
    Free,
    None,
}

impl GoldCategory {
    fn half_units(self) -> usize {
        match self {
            Self::Limited | Self::Free => 2,
            Self::Standard => 1,
            Self::None => 0,
        }
    }
}

/// 原始光锥练度及可选的显式光锥 ID。
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightConeSpec {
    pub light_cone_id: Option<String>,
    pub level: u8,
    pub promotion: u8,
    pub superimposition: u8,
}

impl Default for LightConeSpec {
    fn default() -> Self {
        Self {
            light_cone_id: None,
            level: 80,
            promotion: 6,
            superimposition: 1,
        }
    }
}

/// 覆盖角色原始练度和配装。对应角色加入必需持有人选。
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterSpec {
    pub level: u8,
    pub promotion: u8,
    pub eidolons: u8,
    pub traces: Option<TraceConfig>,
    pub light_cone: LightConeSpec,
    pub relics: Option<RelicRequest>,
    pub relic_quality_tier: Option<u8>,
}

impl Default for CharacterSpec {
    fn default() -> Self {
        Self {
            level: 80,
            promotion: 6,
            eidolons: 0,
            traces: None,
            light_cone: LightConeSpec::default(),
            relics: None,
            relic_quality_tier: None,
        }
    }
}

/// 账号规模、预算、核心角色和遗器质量的约束。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRequest {
    pub account_id: String,
    pub character_count: usize,
    pub gold_count: Option<f64>,
    pub five_star_build: Option<(u8, u8)>,
    pub character_pool: Option<Vec<String>>,
    #[serde(default)]
    pub core_character_ids: Vec<String>,
    #[serde(default = "default_core_count")]
    pub core_count: usize,
    #[serde(default)]
    pub character_overrides: BTreeMap<String, CharacterSpec>,
    pub light_cone_pool: Option<Vec<String>>,
    #[serde(default = "default_gold_categories")]
    pub gold_categories: Vec<GoldCategory>,
    #[serde(default)]
    pub resource_concentration: f64,
    pub gold_concentration: Option<f64>,
    pub relic_concentration: Option<f64>,
    #[serde(default = "default_teammate_bias")]
    pub teammate_bias: f64,
    #[serde(default = "default_light_cone_share")]
    pub light_cone_gold_share: f64,
    pub min_mean_effective_rolls: Option<f64>,
    pub max_mean_effective_rolls: Option<f64>,
    pub relic_quality_tier: Option<u8>,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
}

fn default_core_count() -> usize {
    1
}
fn default_gold_categories() -> Vec<GoldCategory> {
    vec![GoldCategory::Limited, GoldCategory::Standard]
}
fn default_teammate_bias() -> f64 {
    0.7
}
fn default_light_cone_share() -> f64 {
    0.25
}
fn default_max_attempts() -> usize {
    1_000_000
}

impl AccountRequest {
    pub fn new(account_id: impl Into<String>, character_count: usize) -> Self {
        Self {
            account_id: account_id.into(),
            character_count,
            gold_count: None,
            five_star_build: None,
            character_pool: None,
            core_character_ids: Vec::new(),
            core_count: default_core_count(),
            character_overrides: BTreeMap::new(),
            light_cone_pool: None,
            gold_categories: default_gold_categories(),
            resource_concentration: 0.0,
            gold_concentration: None,
            relic_concentration: None,
            teammate_bias: default_teammate_bias(),
            light_cone_gold_share: default_light_cone_share(),
            min_mean_effective_rolls: None,
            max_mean_effective_rolls: None,
            relic_quality_tier: None,
            max_attempts: default_max_attempts(),
        }
    }

    fn validate(&self) -> Result<(), GenerationError> {
        use std::collections::BTreeSet;
        if self.account_id.trim().is_empty() {
            return Err(invalid("账号 ID 不能为空"));
        }
        if self
            .gold_count
            .is_some_and(|gold| !gold.is_finite() || gold < 0.0 || (gold * 2.0).fract() != 0.0)
        {
            return Err(invalid("金数需要非负，并以 0.5 金为步长"));
        }
        let categories: BTreeSet<_> = self.gold_categories.iter().copied().collect();
        if categories.len() != self.gold_categories.len()
            || categories.contains(&GoldCategory::None)
        {
            return Err(invalid("金数类别采用互异的 limited、standard 或 free"));
        }
        if [
            Some(self.resource_concentration),
            self.gold_concentration,
            self.relic_concentration,
            Some(self.teammate_bias),
            Some(self.light_cone_gold_share),
        ]
        .into_iter()
        .flatten()
        .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(invalid("集中程度、队友关联和光锥份额取 0～1"));
        }
        if self.core_character_ids.is_empty()
            && self.character_count > 0
            && self.core_count > self.character_count
        {
            return Err(invalid("核心数量超过持有人数"));
        }
        if self
            .five_star_build
            .is_some_and(|(eidolons, stack)| eidolons > 6 || !(1..=5).contains(&stack))
        {
            return Err(invalid("统一五星配装的星魂取 0～6，光锥叠影取 1～5"));
        }
        relics::validate_range(self.min_mean_effective_rolls, self.max_mean_effective_rolls)?;
        relics::validate_quality(self.relic_quality_tier)?;
        for spec in self.character_overrides.values() {
            spec.validate()?;
        }
        Ok(())
    }
}

impl CharacterSpec {
    fn validate(&self) -> Result<(), GenerationError> {
        const LEVELS: [(u8, u8); 7] = [
            (1, 20),
            (20, 30),
            (30, 40),
            (40, 50),
            (50, 60),
            (60, 70),
            (70, 80),
        ];
        for (level, promotion) in [
            (self.level, self.promotion),
            (self.light_cone.level, self.light_cone.promotion),
        ] {
            if !LEVELS
                .get(usize::from(promotion))
                .is_some_and(|&(min, max)| (min..=max).contains(&level))
            {
                return Err(invalid("角色与光锥的原始等级需匹配突破阶段"));
            }
        }
        if self.eidolons > 6 || !(1..=5).contains(&self.light_cone.superimposition) {
            return Err(invalid("星魂数取 0～6，光锥叠影取 1～5"));
        }
        relics::validate_quality(self.relic_quality_tier)?;
        if let Some(relics) = &self.relics {
            relics.validate()?;
        }
        Ok(())
    }
}

/// 每名角色的五星份数、按部位排列的遗器得分及配装理论上限。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterInvestment {
    pub copies_by_category: BTreeMap<GoldCategory, usize>,
    pub piece_scores: [f64; 6],
    pub relic_score_upper: f64,
}

/// 原始账号直接用于配置转换，生成统计供研究调用方使用。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneratedAccount {
    pub account: RawAccount,
    pub generator_version: &'static str,
    pub core_character_ids: Vec<String>,
    pub investments: BTreeMap<String, CharacterInvestment>,
    pub gold_categories: Vec<GoldCategory>,
    pub attempts: usize,
}

impl GeneratedAccount {
    pub fn copies_by_category(&self) -> BTreeMap<GoldCategory, usize> {
        use GoldCategory::{Free, Limited, Standard};
        let mut copies = BTreeMap::from([(Limited, 0), (Standard, 0), (Free, 0)]);
        for investment in self.investments.values() {
            for (&category, &quantity) in &investment.copies_by_category {
                if let Some(total) = copies.get_mut(&category) {
                    *total += quantity;
                }
            }
        }
        copies
    }

    pub fn gold_by_category(&self) -> BTreeMap<GoldCategory, f64> {
        let copies = self.copies_by_category();
        self.gold_categories
            .iter()
            .map(|&category| {
                (
                    category,
                    (copies.get(&category).copied().unwrap_or_default() * category.half_units())
                        as f64
                        / 2.0,
                )
            })
            .collect()
    }

    pub fn gold_count(&self) -> f64 {
        self.investments
            .values()
            .map(|investment| self.gold_units(investment))
            .sum::<usize>() as f64
            / 2.0
    }

    pub fn mean_effective_rolls(&self) -> f64 {
        fraction(
            self.investments.values().map(relic_score).sum(),
            self.account.relics.len() as f64,
        )
    }

    pub fn core_gold_share(&self) -> f64 {
        let core = self
            .core_character_ids
            .iter()
            .map(|id| self.gold_units(&self.investments[id]))
            .sum::<usize>();
        fraction(core as f64 / 2.0, self.gold_count())
    }

    pub fn core_relic_share(&self) -> f64 {
        fraction(
            self.core_character_ids
                .iter()
                .map(|id| relic_score(&self.investments[id]))
                .sum(),
            self.investments.values().map(relic_score).sum(),
        )
    }

    fn gold_units(&self, investment: &CharacterInvestment) -> usize {
        self.gold_categories
            .iter()
            .map(|category| {
                investment
                    .copies_by_category
                    .get(category)
                    .copied()
                    .unwrap_or_default()
                    * category.half_units()
            })
            .sum()
    }
}

fn relic_score(investment: &CharacterInvestment) -> f64 {
    investment.piece_scores.iter().sum()
}

fn fraction(value: f64, total: f64) -> f64 {
    if total == 0.0 { 0.0 } else { value / total }
}

/// 区分输入错误、当前目标无解以及未完成搜索。
#[derive(Debug)]
pub enum GenerationError {
    InvalidRequest(String),
    Reference(String),
    NoSolution,
    BudgetExhausted { attempts: usize },
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) | Self::Reference(message) => {
                formatter.write_str(message)
            }
            Self::NoSolution => formatter.write_str("账号资源条件没有可用分配"),
            Self::BudgetExhausted { attempts } => {
                write!(formatter, "搜索预算耗尽，已尝试 {attempts} 次分配")
            }
        }
    }
}

impl std::error::Error for GenerationError {}

fn invalid(message: impl Into<String>) -> GenerationError {
    GenerationError::InvalidRequest(message.into())
}

struct Search<'a> {
    rng: &'a mut dyn RngCore,
    attempts: usize,
    limit: usize,
}

impl Search<'_> {
    fn consider(&mut self) -> Result<(), GenerationError> {
        if self.attempts >= self.limit {
            return Err(GenerationError::BudgetExhausted {
                attempts: self.attempts,
            });
        }
        self.attempts += 1;
        Ok(())
    }

    fn unit(&mut self) -> f64 {
        (self.rng.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn index(&mut self, len: usize) -> usize {
        let bound = len as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.rng.next_u64();
            if value >= threshold {
                return (value % bound) as usize;
            }
        }
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let swap = self.index(index + 1);
            values.swap(index, swap);
        }
    }
}

/// 生成满足约束的账号，推进调用方传入的随机源。
pub fn generate_account(
    request: &AccountRequest,
    reference: &AccountReference,
    rng: &mut dyn RngCore,
) -> Result<GeneratedAccount, GenerationError> {
    request.validate()?;
    let mut search = Search {
        rng,
        attempts: 0,
        limit: request.max_attempts,
    };
    allocation::select(request, reference, &mut search)
}

/// 生成仅持有指定角色的原始账号，默认从该角色的推荐光锥中抽取。
pub fn generate_character(
    config_id: impl Into<String>,
    character_id: &str,
    spec: &CharacterSpec,
    reference: &AccountReference,
    max_attempts: usize,
    rng: &mut dyn RngCore,
) -> Result<GeneratedAccount, GenerationError> {
    let profile = reference
        .characters
        .get(character_id)
        .ok_or_else(|| invalid(format!("未知角色 {character_id}")))?;
    let mut spec = spec.clone();
    let mut request = AccountRequest::new(config_id, 1);
    request.max_attempts = max_attempts;
    if spec.light_cone.light_cone_id.is_none() {
        if profile.recommended.is_empty() {
            return Err(invalid(format!("角色 {character_id} 缺少推荐光锥")));
        }
        let mut search = Search {
            rng,
            attempts: 0,
            limit: max_attempts,
        };
        spec.light_cone.light_cone_id =
            Some(profile.recommended[search.index(profile.recommended.len())].clone());
    }
    request.character_pool = Some(vec![character_id.to_owned()]);
    request
        .character_overrides
        .insert(character_id.to_owned(), spec);
    generate_account(&request, reference, rng)
}

fn assemble(
    request: &AccountRequest,
    reference: &AccountReference,
    choices: &[allocation::Choice<'_>],
    core_character_ids: Vec<String>,
    search: &mut Search<'_>,
) -> Result<GeneratedAccount, GenerationError> {
    let gear = relics::generate(request, choices, &core_character_ids, search)?;
    let mut account = RawAccount {
        schema_version: RAW_ACCOUNT_SCHEMA_VERSION,
        config_id: request.account_id.clone(),
        game_version: Some(reference.game_version.clone()),
        data_snapshot: reference.data_snapshot.clone(),
        characters: Vec::with_capacity(choices.len()),
        light_cones: Vec::with_capacity(choices.len()),
        relics: Vec::with_capacity(choices.len() * 6),
    };
    let mut investments = BTreeMap::new();
    for (choice, gear) in choices.iter().zip(gear) {
        let allocation::Choice {
            profile,
            spec,
            build,
        } = choice;
        let id = &profile.id;
        let prefix = format!("{}:{id}", request.account_id);
        let cone_instance_id = format!("{prefix}:light-cone:1");
        account.light_cones.push(LightConeInstance {
            instance_id: cone_instance_id.clone(),
            light_cone_id: build.cone.id.clone(),
            level: spec.light_cone.level,
            promotion: spec.light_cone.promotion,
            superimposition: build.stack,
        });
        let mut relic_instance_ids = BTreeMap::new();
        let mut piece_scores = [0.0; 6];
        for (mut relic, score) in gear.pieces {
            let slot = u8::from(relic.slot);
            relic.instance_id = format!("{prefix}:relic:{slot}");
            relic_instance_ids.insert(relic.slot, relic.instance_id.clone());
            piece_scores[usize::from(slot - 1)] = score;
            account.relics.push(relic);
        }
        account.characters.push(CharacterConfig {
            character_id: id.clone(),
            level: spec.level,
            promotion: spec.promotion,
            eidolons: build.eidolons,
            ability_version: profile.ability_version,
            traces: profile.generate_traces(spec)?,
            light_cone_instance_id: Some(cone_instance_id),
            relic_instance_ids,
        });
        let mut copies = BTreeMap::new();
        for (category, quantity) in [
            (profile.category, build.eidolons + 1),
            (build.cone.category, build.stack),
        ] {
            if category != GoldCategory::None {
                *copies.entry(category).or_default() += usize::from(quantity);
            }
        }
        investments.insert(
            id.clone(),
            CharacterInvestment {
                copies_by_category: copies,
                piece_scores,
                relic_score_upper: gear.upper,
            },
        );
    }
    Ok(GeneratedAccount {
        account,
        generator_version: GENERATOR_VERSION,
        core_character_ids,
        investments,
        gold_categories: request.gold_categories.clone(),
        attempts: search.attempts,
    })
}
