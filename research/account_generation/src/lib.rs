//! 按指定资源条件生成可直接交给 account-core 的原始账号。

#![forbid(unsafe_code)]

mod allocation;
/// 参考资料加载、校验及行迹配置生成。
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

/// 五星角色和光锥的投入类别，计数单位为一份本体、星魂或叠影。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldCategory {
    /// 限定五星投入，每份计一金。
    Limited,
    /// 常驻五星投入，每份计半金。
    Standard,
    /// 免费五星投入，显式纳入预算时每份计一金。
    Free,
    /// 不计入五星金数的条目。
    None,
}

/// 提供金数预算折算所需的半金整数单位。
impl GoldCategory {
    /// 返回该类别每份占用的半金单位数。
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
    /// 指定已收录的光锥 ID。留空时，[`generate_character`] 从推荐列表抽取，[`generate_account`] 在池内按命途筛选。
    pub light_cone_id: Option<String>,
    /// 光锥原始等级。
    pub level: u8,
    /// 光锥突破阶段。
    pub promotion: u8,
    /// 光锥叠影数。
    pub superimposition: u8,
}

/// 定义未显式提供光锥练度时使用的默认值。
impl Default for LightConeSpec {
    /// 返回 80 级、6 阶突破、1 叠且未指定 ID 的光锥配置。
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
///
/// `traces` 留空时，根据参考资料中的行迹前置和角色等级生成；显式设置时原样使用。
/// 角色级 `relic_quality_tier` 约束该角色得分，同时账号级档位仍约束全账号总分。
/// `relics` 提供该角色的遗器请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterSpec {
    /// 角色原始等级。
    pub level: u8,
    /// 角色突破阶段。
    pub promotion: u8,
    /// 星魂数。
    pub eidolons: u8,
    /// 显式行迹配置；留空时按参考资料和练度解锁。
    pub traces: Option<TraceConfig>,
    /// 光锥练度及可选 ID。
    pub light_cone: LightConeSpec,
    /// 角色专属遗器请求；未设置时使用推荐配装。
    pub relics: Option<RelicRequest>,
    /// 角色遗器质量档位；与账号级总分档位同时生效。
    pub relic_quality_tier: Option<u8>,
}

/// 定义角色覆盖未显式设置时的默认练度和装备。
impl Default for CharacterSpec {
    /// 返回 80 级角色、0 星魂、默认光锥及未覆盖行迹与遗器的配置。
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
///
/// `gold_count` 指定精确总金数；留空时只受类别、候选和配装约束。
/// 显式核心名单优先于 `core_count`，并作为必需持有人纳入角色候选。
/// 金数和遗器集中度可分别设置；各自留空时沿用 `resource_concentration`。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRequest {
    /// 写入原始账号并用作装备实例前缀的 ID。省略时在配装完成后从调用方的随机源生成。
    pub account_id: Option<String>,
    /// 持有角色总数，允许为零。
    pub character_count: usize,
    /// 精确五星总金数，按半金步长；留空时由其他限制决定。
    pub gold_count: Option<f64>,
    /// 无单独覆盖的五星角色使用其专锥。二元组分别指定角色星魂（0–6）和专锥叠影（1–5）。
    pub five_star_build: Option<(u8, u8)>,
    /// 允许持有的角色 ID 池；留空使用全部参考角色。
    pub character_pool: Option<Vec<String>>,
    /// 必须持有且作为核心统计对象的角色 ID。
    #[serde(default)]
    pub core_character_ids: Vec<String>,
    /// 未显式指定核心名单时自动选择的核心角色数。
    #[serde(default = "default_core_count")]
    pub core_count: usize,
    /// 按角色 ID 设置的完整练度及配装覆盖。
    #[serde(default)]
    pub character_overrides: BTreeMap<String, CharacterSpec>,
    /// 自动配装可选的光锥 ID 池；留空使用全部参考光锥。
    pub light_cone_pool: Option<Vec<String>>,
    /// 计入金数预算的五星类别。
    #[serde(default = "default_gold_categories")]
    pub gold_categories: Vec<GoldCategory>,
    /// 金数和遗器共享的资源集中程度，范围为 0 到 1。
    #[serde(default)]
    pub resource_concentration: f64,
    /// 单独指定金数集中程度；留空沿用共享值。
    pub gold_concentration: Option<f64>,
    /// 单独指定遗器集中程度；留空沿用共享值。
    pub relic_concentration: Option<f64>,
    /// 推荐队友对角色抽取概率的影响程度，范围为 0 到 1。
    #[serde(default = "default_teammate_bias")]
    pub teammate_bias: f64,
    /// 配装分配中光锥金数占角色投入的目标比例。
    #[serde(default = "default_light_cone_share")]
    pub light_cone_gold_share: f64,
    /// 全部已生成遗器的平均有效词条数下限。
    pub min_mean_effective_rolls: Option<f64>,
    /// 全部已生成遗器的平均有效词条数上限。
    pub max_mean_effective_rolls: Option<f64>,
    /// 全账号遗器总分质量档位；角色级档位另行约束对应角色得分。
    pub relic_quality_tier: Option<u8>,
    /// 角色、金数和遗器搜索共用的最大候选考察次数。
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
}

/// 返回自动核心角色数量的反序列化默认值。
fn default_core_count() -> usize {
    1
}
/// 返回默认纳入金数预算的五星类别。
fn default_gold_categories() -> Vec<GoldCategory> {
    vec![GoldCategory::Limited, GoldCategory::Standard]
}
/// 返回推荐队友关联强度的默认值。
fn default_teammate_bias() -> f64 {
    0.7
}
/// 返回光锥金数目标份额的默认值。
fn default_light_cone_share() -> f64 {
    0.25
}
/// 返回搜索预算的默认考察次数。
fn default_max_attempts() -> usize {
    1_000_000
}

/// 创建和校验账号生成请求。
impl AccountRequest {
    /// 以指定账号 ID 和持有人数创建请求。
    pub fn new(account_id: impl Into<String>, character_count: usize) -> Self {
        Self {
            account_id: Some(account_id.into()),
            ..Self::with_random_id(character_count)
        }
    }

    /// 以给定持有人数创建请求。生成时随机命名账号，默认自动选一名核心，金数和遗器分数均不设目标。
    pub fn with_random_id(character_count: usize) -> Self {
        Self {
            account_id: None,
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

    /// 验证请求的金数步长、类别、比例、人数、配装和遗器约束。
    fn validate(&self) -> Result<(), GenerationError> {
        use std::collections::BTreeSet;
        if self
            .account_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
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

/// 校验单个角色覆盖项。
impl CharacterSpec {
    /// 检查等级与突破阶段、星魂、叠影及遗器覆盖是否有效。
    fn validate(&self) -> Result<(), GenerationError> {
        /// 以突破阶段为索引的角色及光锥等级闭区间。
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
/// `piece_scores` 按头、手、躯干、脚、位面球、连结绳排列。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterInvestment {
    /// 此角色本体和光锥按类别统计的五星份数。
    pub copies_by_category: BTreeMap<GoldCategory, usize>,
    /// 按遗器部位次序排列的有效词条得分。
    pub piece_scores: [f64; 6],
    /// 按合法词条和允许的数值档位计算的六件理论最高分，供换算练度档位。
    pub relic_score_upper: f64,
}

/// 原始账号直接用于 account-core 配置转换，生成统计供研究调用方使用。
///
/// `account` 保存原始练度、行迹、装备实例及引用；`investments` 和 `attempts`
/// 记录资源分配与搜索过程，不参与原始账号转换。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneratedAccount {
    /// 可交给 account-core 配置转换的统一原始账号。
    pub account: RawAccount,
    /// 核心角色 ID 列表。
    pub core_character_ids: Vec<String>,
    /// 按角色 ID 索引的五星投入与遗器分数统计。
    pub investments: BTreeMap<String, CharacterInvestment>,
    /// 本次总金数统计纳入的类别。
    pub gold_categories: Vec<GoldCategory>,
    /// 搜索过程中实际考察的候选次数。
    pub attempts: usize,
}

/// 汇总生成账号中的角色投入及遗器统计指标。
impl GeneratedAccount {
    /// 汇总各五星投入类别的本体、星魂和叠影份数。
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

    /// 返回按账号金数类别折算的投入；未计入预算的类别不出现在结果中。
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

    /// 返回所选类别的总金数，常驻五星每份折算半金。
    pub fn gold_count(&self) -> f64 {
        self.investments
            .values()
            .map(|investment| self.gold_units(investment))
            .sum::<usize>() as f64
            / 2.0
    }

    /// 返回账号全部遗器的平均有效词条数；无遗器时为零。
    pub fn mean_effective_rolls(&self) -> f64 {
        fraction(
            self.investments.values().map(relic_score).sum(),
            self.account.relics.len() as f64,
        )
    }

    /// 返回核心角色所占的金数比例；总金数为零时为零。
    pub fn core_gold_share(&self) -> f64 {
        let core = self
            .core_character_ids
            .iter()
            .map(|id| self.gold_units(&self.investments[id]))
            .sum::<usize>();
        fraction(core as f64 / 2.0, self.gold_count())
    }

    /// 返回核心角色所占的遗器得分比例；全账号遗器得分为零时为零。
    pub fn core_relic_share(&self) -> f64 {
        fraction(
            self.core_character_ids
                .iter()
                .map(|id| relic_score(&self.investments[id]))
                .sum(),
            self.investments.values().map(relic_score).sum(),
        )
    }

    /// 计算一个角色投入中按请求计入的半金单位数。
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

/// 汇总单个角色六件遗器的有效词条分数。
fn relic_score(investment: &CharacterInvestment) -> f64 {
    investment.piece_scores.iter().sum()
}

/// 计算比例；分母为零时返回零。
fn fraction(value: f64, total: f64) -> f64 {
    if total == 0.0 { 0.0 } else { value / total }
}

/// 区分输入错误、当前目标无解以及未完成搜索。
#[derive(Debug)]
pub enum GenerationError {
    /// 请求中的字段取值无效。
    InvalidRequest(#[doc = "说明请求字段无效原因的消息。"] String),
    /// 参考资料缺失、格式错误或内部条件不一致。
    Reference(#[doc = "说明参考资料错误原因的消息。"] String),
    /// 输入有效，但约束没有可行分配。
    NoSolution,
    /// 尚未找到解便用尽搜索预算。
    BudgetExhausted {
        /// 搜索预算耗尽前已考察的候选次数。
        attempts: usize,
    },
}

/// 将生成错误格式化为可读消息。
impl fmt::Display for GenerationError {
    /// 输出错误变体所携带的消息或搜索耗尽次数。
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

/// 允许通过标准错误接口传播生成失败。
impl std::error::Error for GenerationError {}

/// 创建带具体说明的无效请求错误。
fn invalid(message: impl Into<String>) -> GenerationError {
    GenerationError::InvalidRequest(message.into())
}

/// 对随机整数做拒绝采样，在 `0..len` 内均匀选一个下标。调用方须传入非零长度。
fn random_index(rng: &mut dyn RngCore, len: usize) -> usize {
    let bound = len as u64;
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = rng.next_u64();
        if value >= threshold {
            return (value % bound) as usize;
        }
    }
}

/// 在一次生成过程中共享随机源和候选考察预算。
struct Search<'a> {
    /// 调用方传入并由搜索过程持续推进的随机数源。
    rng: &'a mut dyn RngCore,
    /// 已经计入预算的候选考察次数。
    attempts: usize,
    /// 本次请求允许考察的候选次数上限。
    limit: usize,
}

/// 提供统一的候选计数和随机抽样操作。
impl Search<'_> {
    /// 消耗一次候选考察预算，超限时返回预算耗尽错误。
    fn consider(&mut self) -> Result<(), GenerationError> {
        if self.attempts >= self.limit {
            return Err(GenerationError::BudgetExhausted {
                attempts: self.attempts,
            });
        }
        self.attempts += 1;
        Ok(())
    }

    /// 用一个随机 `u64` 的高 53 位得到 `[0, 1)` 内的浮点数。
    fn unit(&mut self) -> f64 {
        (self.rng.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// 从指定长度范围内均匀选择一个索引。
    fn index(&mut self, len: usize) -> usize {
        random_index(self.rng, len)
    }

    /// 使用 Fisher–Yates 算法原地打乱切片。
    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let swap = self.index(index + 1);
            values.swap(index, swap);
        }
    }
}

/// 生成满足约束的账号，推进调用方传入的随机源。
///
/// 请求字段无效时返回 [`GenerationError::InvalidRequest`]，搜索确认约束无解时返回
/// [`GenerationError::NoSolution`]。最后一次允许的尝试可以成功。若仍需尝试下一个候选，
/// [`GenerationError::BudgetExhausted`] 给出已用次数，此时可能仍有可行分配。
///
/// 复现结果时需使用相同请求、参考资料和随机源初始状态；连续调用共享随机源时，
/// 后续结果还取决于此前调用的顺序及消耗。可保存调用前状态或从种子重放调用序列。
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
/// 显式指定 `spec.light_cone.light_cone_id` 时使用该光锥；`max_attempts` 限制搜索预算。
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
        let index = random_index(rng, profile.recommended.len());
        spec.light_cone.light_cone_id = Some(profile.recommended[index].clone());
    }
    request.character_pool = Some(vec![character_id.to_owned()]);
    request
        .character_overrides
        .insert(character_id.to_owned(), spec);
    generate_account(&request, reference, rng)
}

/// 将已选角色和装备组装成 RawAccount，并计算每名角色的投入统计。
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
        config_id: request.account_id.clone().unwrap_or_else(|| {
            format!(
                "account-{:016x}{:016x}",
                search.rng.next_u64(),
                search.rng.next_u64()
            )
        }),
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
        let prefix = format!("{}:{id}", account.config_id);
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
            traces: profile.generate_traces(spec),
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
        core_character_ids,
        investments,
        gold_categories: request.gold_categories.clone(),
        attempts: search.attempts,
    })
}
