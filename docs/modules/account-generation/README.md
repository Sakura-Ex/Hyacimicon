# 账号生成器

`account-generator` 用参考资料、生成条件和随机源生成模拟账号。输出的 `RawAccount` 保存持有角色、光锥与遗器实例，以及装备关系。生成器位于 `research/account_generation/`，构建时嵌入 4.7 参考资料。

## 运行默认样例

在仓库根目录用 Cargo 运行：

```powershell
cargo run --locked -p account-generator --example generate_account
```

默认请求使用种子 42，生成 12 名角色、20 金、每件遗器平均有效词条 4.5～5 的账号。标准输出是 `RawAccount` JSON，可检查 `game_version` 为 `4.7`、`characters` 有 12 项。标准错误最后一行显示：

```text
seed=42 gold=20 mean_effective_rolls=4.5333 attempts=94
```

要计算账号在货币战争中实际生效的配置，将输出读入 `RawAccount`，连同对应版本的转换资料和转换请求传给 `convert_playable_account`。转换输入、试用补齐和版本要求见[实际配置转换](data-flow.md)。

## 按任务查阅

| 要做什么 | 文档 |
| --- | --- |
| 指定人数、金数、全 01／全 21 或资源分布，复现账号样本 | [账号生成](account-generation.md) |
| 生成指定角色的原始等级、行迹、光锥和六件遗器 | [完整角色生成](character-generation.md) |
| 生成能满足整套总分的六件遗器 | [六件遗器生成](character-relics.md) |
| 生成单件遗器或复算单件分数 | [单件遗器生成与评分](relic-generation.md) |
| 把低练到毕业的五档换算为得分范围 | [遗器练度五档](relic-quality.md) |
| 核对推荐数据、生成规则与装备实例限制 | [生成设计](design.md) |
| 接入实际配置转换 | [账号数据流](data-flow.md) |

查阅游戏条目时使用[角色数据](../../game-data/character-data.md)、[光锥数据](../../game-data/light-cone-data.md)和[五星遗器数据](../../game-data/relic-data.md)。
