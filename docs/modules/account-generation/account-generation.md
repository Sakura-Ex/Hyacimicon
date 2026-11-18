# 账号数据生成

Rust crate `account-generator` 使用内置 4.7 参考资料离线生成角色、光锥、遗器和装备关系。`GeneratedAccount.account` 是 `account_core::RawAccount`，可直接交给 [`convert_playable_account`](../account/playable-configuration.md)。

## 运行样例

在仓库根目录运行默认样例。它使用种子 42 生成 12 人、20 金、平均有效词条 4.5～5 的账号。

```powershell
cargo run --locked -p account-generator --example generate_account
```

示例在标准输出写入 `RawAccount` JSON，在标准错误输出种子、生成器版本、实际金数、平均词条与搜索次数。准备请求文件后可指定参数和种子。

```json
{
  "account_id": "account-001",
  "character_count": 12,
  "gold_count": 20,
  "core_character_ids": ["1310", "1225"],
  "resource_concentration": 0.7,
  "min_mean_effective_rolls": 4.5,
  "max_mean_effective_rolls": 5
}
```

```powershell
cargo run --locked -p account-generator --example generate_account -- request.json 42
```

## 调用 Rust 接口

```rust
use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, SeedableRng, generate_account,
};

let reference = AccountReference::bundled()?;
let mut request = AccountRequest {
    gold_count: Some(20.0),
    core_character_ids: vec!["1310".into(), "1225".into()],
    resource_concentration: 0.7,
    min_mean_effective_rolls: Some(4.5),
    max_mean_effective_rolls: Some(5.0),
    ..AccountRequest::new("account-001", 12)
};
let mut rng = ChaCha8Rng::seed_from_u64(42);
let result = generate_account(&request, &reference, &mut rng)?;
```

参考资料可复用。`AccountReference::from_dir` 可加载包含 `source-manifest.json`、`character.json`、`character/` 和 `lightcone.json` 的外部快照。加载时逐项校验角色的推荐主词条与套装，错误信息包含角色 ID。`ReferenceOverrides` 支持覆盖角色和光锥分类、五星专锥及原始能力版本。覆盖的五星类别须与条目稀有度相符，统一配装的专锥须为同命途五星光锥。加载时检查行迹前置节点及循环。复现时保存请求、随机源与种子、生成器版本、实际使用的参考文件及完整的覆盖设置。文件也可用唯一快照 ID 或内容哈希标识。

## 生成条件

| 参数 | 含义 | 默认值 |
| --- | --- | --- |
| `account_id` | 账号 ID，也是装备实例 ID 前缀 | 必填 |
| `character_count` | 持有角色总数 | 必填，允许 0 |
| `gold_count` | 精确金数，支持 0.5 金步长 | 从可达金额中选择 |
| `five_star_build` | 所有持有五星的统一星魂和专锥叠影，如 `(0, 1)`、`(2, 1)` | `None` |
| `character_pool` | 允许持有的常规角色 ID；数量等于候选数时选取全部 | `None`，使用全部参考角色 |
| `core_character_ids` | 指定核心角色，保证持有 | `()` |
| `core_count` | 未指定核心时，自动选择的核心数量 | 1，空账号为 0 |
| `character_overrides` | 角色 ID 到 `CharacterSpec` 的完整覆盖，并保证持有 | `{}` |
| `light_cone_pool` | 自动配装可选的光锥 ID，按角色命途筛选 | `None`，使用全部同命途候选 |
| `gold_categories` | 计入预算的资源类别 | `("limited", "standard")` |
| `resource_concentration` | 同时控制金数和遗器的集中程度，取 0～1 | 0 |
| `gold_concentration` | 独立覆盖金数集中程度 | `None`，沿用共同值 |
| `relic_concentration` | 独立覆盖遗器集中程度 | `None`，沿用共同值 |
| `teammate_bias` | 推荐队友关联强度，取 0～1 | 0.7 |
| `light_cone_gold_share` | 配装搜索中光锥金数占角色投入的目标比例 | 0.25 |
| `min_mean_effective_rolls` | 全部已装备遗器的每件平均有效词条下限 | `None` |
| `max_mean_effective_rolls` | 同一范围的平均有效词条上限 | `None` |
| `relic_quality_tier` | 按账号理论总上限换算的五档遗器目标 | `None` |
| `max_attempts` | 角色选择、金数分配和遗器搜索共用的候选考察预算 | 1000000 |

核心和覆盖角色需要在候选范围内，持有人数需要能容纳这些角色。未提供固定练度的角色，在预算内选择星魂 0～6、光锥叠影 1～5；原始等级和突破采用角色生成的 80 级、阶段 6 默认值。

每名持有角色生成一个光锥实例和六件五星 +15 遗器。金数容量由持有人数、星魂上限、光锥叠影及显式覆盖共同决定。

## 金数与统一五星配装

五星角色每个本体及星魂对应一份，五星光锥每级叠影对应一份：

| 类别 | 标识 | 每份折算 | 默认计入预算 |
| --- | --- | ---: | --- |
| 限定 | `limited` | 1 金 | 是 |
| 常驻 | `standard` | 0.5 金 | 是 |
| 免费五星 | `free` | 显式纳入时按 1 金统计 | 否 |
| 四星及以下 | `none` | 0 金 | 否 |

`gold_count` 指定时，输出的实际金数与其相同。限定、常驻和免费的持有份数分别保留，常驻折算为半金。例如常驻角色 2 魂配常驻光锥叠影 1，合计四份，折算 2 金。

默认分类由 [`reference.rs`](../../../research/account_generation/src/reference.rs) 维护：常驻角色为 1003、1004、1101、1104、1107、1209、1211；常驻光锥为 23000、23002、23003、23004、23005、23012、23013。80xx 主角形态和 24xxx 模拟宇宙五星光锥归免费，其余五星归限定。构造 `ReferenceOverrides` 时，可用 `character_categories`、`light_cone_categories` 覆盖具体条目的类别。

统一配装可独立使用，也可与金数条件共同使用：

```rust
let all_01 = AccountRequest { gold_count: None, five_star_build: Some((0, 1)), ..request.clone() };
let all_21 = AccountRequest { gold_count: None, five_star_build: Some((2, 1)), ..request.clone() };
```

四星角色按自身条件生成。统一配装优先使用加载资料时指定的 `ReferenceOverrides.signature_light_cones`，默认取推荐列表中第一件同命途五星光锥。缺少这类推荐时，应通过覆盖设置指定同命途五星光锥。

统一配装固定的投入先得到满足，集中程度再控制其他可分配投入。金数与统一配装冲突时，生成器报告条件无法同时满足。

## 角色与光锥覆盖

`CharacterSpec` 与[单角色请求](character-generation.md)共用原始练度字段，包括等级、突破、星魂、行迹、光锥、遗器请求和单角色遗器档位。账号级覆盖无需填写单角色 `config_id`，实例 ID 由账号统一分配。

```rust
use account_generator::{CharacterSpec, LightConeSpec};

request.character_overrides.insert("1310".into(), CharacterSpec {
    eidolons: 2,
    light_cone: LightConeSpec {
        light_cone_id: Some("23025".into()),
        ..LightConeSpec::default()
    },
    relic_quality_tier: Some(5),
    ..CharacterSpec::default()
});
```

覆盖字段优先于统一五星配装，角色的星魂和光锥叠影采用覆盖值。覆盖角色与其他候选一起参与自动核心选择。覆盖中光锥 ID 留空时，仍在对应候选范围内自动选择。显式传入光锥 ID 时直接采用，支持不同命途或候选池以外的配装；该 ID 需要存在于光锥资料中，供预算统计查询。

自动配装始终匹配命途，在接近投入目标的组合中优先推荐光锥，必要时采用候选池内其他同命途光锥。活动及模拟宇宙光锥每种至多分配一个实例；普通光锥允许同 ID 多个实例，见[光锥持有约定](design.md#光锥持有与装备实例)。

两人都只能使用同一件唯一光锥时，搜索直接跳过该名单。存在替代光锥时继续尝试其他配装。名单搜索对每种持有集合与核心集合的组合考察一次，并记录各人数可达的半金数。候选角色全部持有时，名单的金数可达性只需校验一次。未指定金数时，随机排列可达金额，逐个尝试满足唯一光锥约束的分配。全部搜索共用 `max_attempts` 预算。

## 推荐队友与资源集中程度

推荐资料按原队伍和位置保留。选中角色后，从其固定推荐队伍中抽取一组。每名已选角色在同一支推荐队伍中最多满足一个位置，搜索优先满足尽可能多的位置。尚未满足的位置为候选首选队友加权 1，为对应替补加权 0.5。明确列出的角色 ID 参与抽取。

`teammate_bias=0` 按可行候选等概率选择，`1` 优先从这些队友中选取；中间值混合两种分布。所有选择同时满足人数、候选范围和金数可达条件。

集中程度为 `c`，持有人数为 `n`，核心人数为 `m`。满足各角色下限后，剩余投入按以下权重分配：

- 普通角色：`(1-c)/n`。
- 核心角色：`(1-c)/n + c/m`。

`c=0` 平均分配可调投入，`c=1` 优先投入核心。核心达到容量后，剩余投入分配给其他角色。固定配装、角色本体所需金数和遗器上限都会影响实际份额，结果提供 `core_gold_share` 与 `core_relic_share` 供比较。

金数分配在接近各角色目标投入的组合中，进一步接近 `light_cone_gold_share`；该参数表达光锥投入倾向。

## 遗器目标与返回结果

每名持有角色生成六件五星 +15 遗器。平均得分按全部遗器计算，各件按对应角色权重评分。单件、角色和账号的分数约束共同生效。五档条件按各角色理论上限之和换算，再与显式平均范围和角色级条件取交集。

中档得分可用非负有界整数网格表示时，生成器先合并各件和各角色的可达总分，取得满足条件的中档组合。检查候选时，一旦所有支持的刻度均不适用，就转入普通搜索。之后按允许档位的权重重抽强化数值。等权副词条按随机顺序尝试强化次数分配。零计分词条共用的额外强化次数逐次随机分配，各自独立抽取数值档位，保留评分和整件强化总次数。重抽结果还要满足单件、角色与账号的分数约束。精确目标也允许零权重词条变化。其余目标通过允许档位内的回溯搜索求解，搜索每尝试一件成品扣一次预算。三条或四条起始条件按 5∶2 抽取优先候选。搜索完毕且无可用分配时返回 `NoSolution`，下一件候选需要超出考察上限时返回 `BudgetExhausted`。

`GeneratedAccount` 保存统一原始账号、实际核心名单、逐角色投入统计、统计口径、生成器版本和搜索次数。用 `gold_count()`、`gold_by_category()`、`copies_by_category()`、`mean_effective_rolls()`、`core_gold_share()` 和 `core_relic_share()` 读取汇总值。五星份数按限定、常驻和免费三个类别汇总。原始账号记录游戏版本及外部资料提供的快照标识。空账号的金数和平均词条均为零。

## 验证

```powershell
cargo test --locked -p account-generator
```

集成测试覆盖固定种子、全 01／全 21 配装、唯一光锥冲突、原始练度、宽区间三档抽样、精确分数与零权重词条的数值变化，以及考察上限。12 人窄区间样例与 60 人、400 金、平均词条 6.5 的精确样例均调用实际配置转换。
