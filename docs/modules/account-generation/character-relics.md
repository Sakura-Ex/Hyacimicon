# 角色六件遗器生成

`generate_character_relics` 同时满足单件和整套的有效词条目标，输出六件五星 +15 成品及各件得分。指定的部位按请求顺序返回，其余部位按头、手、躯干、脚、球、绳的顺序补齐。

## 调用

```rust
use std::collections::BTreeMap;
use account_core::{RelicMainStat, RelicSlot, RelicSubStat};
use account_generator::{ChaCha8Rng, RelicRequest, RelicSpec, SeedableRng, generate_character_relics};

let request = RelicRequest {
    pieces: vec![RelicSpec {
        mainstat: vec![RelicMainStat::HpPercent, RelicMainStat::DefPercent],
        ..RelicSpec::new(RelicSlot::PlanarSphere)
    }],
    effective_stat_weights: BTreeMap::from([(RelicSubStat::CritRate, 1.0), (RelicSubStat::Spd, 1.0)]),
    min_total_effective_rolls: Some(12.0),
    max_total_effective_rolls: Some(18.0),
    ..RelicRequest::default()
};
let mut rng = ChaCha8Rng::seed_from_u64(42);
let pieces = generate_character_relics(&request, 100_000, &mut rng)?;
```

| `RelicRequest` 字段 | 含义 |
| --- | --- |
| `pieces` | 可选的部位列表，各部位最多出现一次。部位字段见[单件接口](relic-generation.md) |
| `effective_stat_weights` | 六件共用的角色评分权重，范围和单位见[评分单位与数值限制](relic-generation.md#评分单位与数值限制) |
| `tier_weights` | 低、中、高档的非负权重，默认 `[1.0; 3]`。零权重排除该档，至少一档为正 |
| `min_total_effective_rolls`、`max_total_effective_rolls` | 六件合计分数上下限，各端可独立设置 |

上下限相等时，生成结果在 `1e-6` 的绝对容差内匹配目标总分。单件分数限制和整套限制共同生效，六件共享 `max_attempts`。调用方可按相同请求和随机源重现结果。

`maximum_character_relic_score(&request)` 根据合法的部位、主副词条、起始条数和允许档位计算六件理论上限。该上限用于[五档练度](relic-quality.md)。

返回值为 `Vec<(RelicInstance, f64)>`。组装进账号库存时，由调用方分配实例 ID。需要生成推荐配装和装备引用时，使用[完整角色生成](character-generation.md)。

## 验证

运行 `cargo test --locked -p account-generator`。回归用例覆盖部位顺序、空缺补齐、混合起始条数、单件和整套合计边界、精确值、不可达值及预算耗尽。
