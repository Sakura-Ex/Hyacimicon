# 单件遗器生成与评分

`generate_relic` 按部位、配装约束和目标分数生成一件五星 +15 遗器，返回成品和得分。属性键、部位与套装 ID 见[五星遗器数据](../../game-data/relic-data.md)。

## 调用

```rust
use std::collections::BTreeMap;
use account_core::{RelicSlot, RelicSubStat};
use account_generator::{ChaCha8Rng, RelicSpec, SeedableRng, generate_relic};

let spec = RelicSpec {
    set_id: Some("115".into()),
    required_substats: vec![RelicSubStat::Spd],
    min_effective_rolls: Some(2.0),
    max_effective_rolls: Some(4.0),
    ..RelicSpec::new(RelicSlot::Head)
};
let weights = BTreeMap::from([(RelicSubStat::Spd, 1.0)]);
let mut rng = ChaCha8Rng::seed_from_u64(42);
let (piece, score) = generate_relic(&spec, &weights, [1.0; 3], 10_000, &mut rng)?;
```

| `RelicSpec` 字段 | 含义 |
| --- | --- |
| `slot` | 固定部位，序列化为数字 1～6 |
| `set_id` | 指定套装 ID。留空时从该部位支持的套装中抽取 |
| `mainstat` | 主词条候选。空列表采用该部位的全部合法主词条 |
| `initial_substat_count` | 起始三条或四条。留空时以 5∶2 的权重选优先候选，两种条件都可参与搜索 |
| `required_substats` | 成品必含的副词条，互异且最多四种 |
| `min_effective_rolls`、`max_effective_rolls` | 单件分数上下限，各端可独立设置 |

`tier_weights` 依次对应低、中、高档，采用至少一个为正的有限非负权重。权重为零的档位不可用。三条起始的成品获得八次数值，四条起始获得九次数值，每种副词条至少出现一次。成品保留四种互异副词条，主副词条遵守排斥规则。

## 计分与错误

`score_relic(&piece, &weights)` 先将各副词条数值除以该属性的单次中档值，再乘对应权重求和。自定义权重可为正值或负值，未列出的属性计零分。百分比属性使用实际比例值，例如 3.24% 保存为 `0.0324`。自动分配实例 ID 的入口见[完整角色配置生成](character-generation.md)。

无效字段返回 `InvalidRequest`，搜索完毕且无可行解时返回 `NoSolution`，需要下一次考察但预算已用完时返回 `BudgetExhausted`。

## 验证

运行 `cargo test --locked -p account-generator`。回归用例覆盖档位的合法分解、零权重档位、主副词条限制、单双边分数、自定义负权重与精确目标。
