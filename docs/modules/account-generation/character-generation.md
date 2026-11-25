# 完整角色配置生成

`generate_character` 生成一个只持有指定角色的 `RawAccount`，包括一个光锥实例、六件五星 +15 遗器和装备引用。使用 [`convert_playable_account`](../account/playable-configuration.md) 展开实际游玩配置。批量持有角色及账号级金数由[账号生成](account-generation.md)处理。

## 调用

```rust
use account_generator::{
    AccountReference, ChaCha8Rng, CharacterSpec, LightConeSpec, ReferenceOverrides, SeedableRng, generate_character,
};
use account_core::reference::TraceCatalog;
use std::path::Path;

let traces = TraceCatalog::from_path(Path::new("docs/game-data/runtime/account/reliquary-traces-v4.json"))?;
let reference = AccountReference::from_dir(Path::new("docs/game-data/runtime/account-generation"), Some(&traces), &ReferenceOverrides::default())?;
let spec = CharacterSpec {
    eidolons: 2,
    light_cone: LightConeSpec {
        light_cone_id: Some("23025".into()),
        ..LightConeSpec::default()
    },
    ..CharacterSpec::default()
};
let mut rng = ChaCha8Rng::seed_from_u64(42);
let result = generate_character("firefly-21", "1310", &spec, &reference, 100_000, &mut rng)?;
let raw = result.account;
```

`config_id` 关联后续实际配置，也是装备实例 ID 的前缀。参考资料可在多次调用间复用。按[账号生成说明](account-generation.md#调用-rust-接口)加载外部快照。

## 角色与光锥

| `CharacterSpec` 字段 | 含义与默认值 |
| --- | --- |
| `level`、`promotion` | 角色原始等级和突破，默认 80 级、突破 6 |
| `eidolons` | 星魂 0～6，默认 0 |
| `traces` | 显式 `TraceConfig`，留空时按等级、突破和前置节点生成 |
| `light_cone` | 光锥选择及练度，默认 80 级、突破 6、叠影 1 |
| `relics` | 可选的 `RelicRequest`，默认采用推荐配装 |
| `relic_quality_tier` | 可选的遗器练度档位 1～5 |

角色和光锥的原始等级都按突破阶段取值。阶段 0～6 依次对应 1～20、20～30、30～40、40～50、50～60、60～70、70～80。光锥叠影允许 1～5。显式设置 `light_cone_id` 时采用指定光锥，留空时从同命途推荐中抽取。

`TraceConfig.skill_levels` 按行迹节点 ID 保存基础等级。同一节点关联的技能共用这个等级。 `node_unlocks` 保存额外能力和属性节点的已知解锁状态。生成器原样保存显式行迹。需要转换时，请填齐该能力版本各多级技能的基础等级，或传入 `traces: None` 让生成器补全。转换器读取这些等级后应用试用保底和星魂加级。

## 遗器覆盖

推荐外圈套装用于头、手、躯干和脚，推荐位面套装用于球和绳。传入 `RelicRequest` 时，已指定部位采用该部位的完整条件，其余部位按推荐补齐。已指定部位的套装 ID 留空时，从相容套装中随机选择。整套使用请求中的 `effective_stat_weights`，留空时所有副词条计零分。部分覆盖配合练度档位使用时，请提供能产生正理论上限的权重。

`relic_quality_tier` 将[五档区间](relic-quality.md)与显式整套得分范围取交集。理论上限由主词条、必含属性、起始条数和允许档位决定。单件与整套分数目标进一步筛选成品。

`GeneratedAccount.account` 保存原始持有配置。`investments[character_id]` 保存按部位排列的分数和理论上限，`attempts` 记录共用的考察次数。

## 验证

运行 `cargo test --locked -p account-generator`。回归用例覆盖原始行迹、光锥叠影、实例引用、部分覆盖、五档练度和配置转换。
