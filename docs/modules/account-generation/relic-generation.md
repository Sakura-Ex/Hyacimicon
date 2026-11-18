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
| `initial_substat_count` | 起始三条或四条。留空时的基础概率分别为 5/7 和 2/7；分数约束可改变成品中的比例 |
| `required_substats` | 成品必含的副词条，互异且最多四种 |
| `min_effective_rolls`、`max_effective_rolls` | 单件分数上下限，各端可独立设置 |

`tier_weights` 依次对应低、中、高档，采用至少一个为正的有限非负权重。权重为零的档位不可用。三条起始的成品获得八次数值，四条起始获得九次数值，每种副词条至少出现一次。成品保留四种互异副词条，主副词条遵守排斥规则。

在兼容的主词条候选中抽取主词条，保留必含副词条，其余位置从合法属性中等概率不放回抽取。成品四条各取得一次初始数值，剩余四次或五次强化逐次独立四选一，每次按档位权重抽数值。评分权重用于计算成品分数。举例来说，头部只给速度计分、未指定必含属性和分数范围时，速度出现的概率仍为 4/11。

指定单件分数范围后，先检验按上述概率抽出的成品。若需要条件抽样，程序按各组合及后续强化能够达标的概率选取，复用相同剩余次数和得分的计算结果。以初始四条、双暴各计 1 分、其余两条计 0 分且只允许中档为例，精确 7 分要求五次强化全部进入双暴。在这个条件下五次全落在同一条的概率为 1/16。

## 计分与错误

`score_relic(&piece, &weights)` 先将各副词条数值除以该属性的单次中档值，再乘对应权重求和。自定义权重可为正值或负值，未列出的属性计零分。百分比属性使用实际比例值，例如 3.24% 保存为 `0.0324`。自动分配实例 ID 的入口见[完整角色配置生成](character-generation.md)。

无效字段返回 `InvalidRequest`，搜索完毕且无可行解时返回 `NoSolution`，需要下一次考察但预算已用完时返回 `BudgetExhausted`。

## 验证

运行 `cargo test --locked -p account-generator`。回归用例覆盖档位的合法分解、零权重档位、主副词条限制、单双边分数、自定义负权重与精确目标。
