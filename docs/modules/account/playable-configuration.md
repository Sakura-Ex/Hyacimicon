# 账号静态配置、试用补齐与保底映射

状态：静态配置转换已实现。适用游戏版本为 4.7，玩法角色范围为货币战争 S2。

## 输入与交付

公开入口 `playable::conversion::convert_playable_account(raw, data, request)` 接收 `RawAccount`、`PlayableData` 和 `ConversionRequest`，输出 `PlayableAccount`。请求包含输出配置 ID 和已核对的角色 ID 对照。角色来源由原始账号的实际持有情况确定，缺少必需资料或资源引用冲突时返回 `ConversionError`。

账号模块完成来源选择、练度保底、技能加级和静态属性计算。局面模拟使用这份配置维护实际阵容。战斗输入组装再根据局面、敌人和游戏资料计算开战属性、固定系数及动作参数；战斗引擎执行战斗流程，实时处理 buff 和触发效果。调用关系见[账号数据流](../account-generation/data-flow.md)。

## 数据结构

类型位于 `core/account/src/playable/`。格式版本 `PLAYABLE_ACCOUNT_SCHEMA_VERSION` 当前为 3。

| 类型 | 保存内容 |
| --- | --- |
| `PlayableAccount` | 配置关联、游戏版本、赛季、资料与规则版本、角色列表和持有库存 |
| `PlayableCharacter` | 角色 ID、本体或试用来源、星魂、能力版本、行迹、光锥、遗器和静态面板 |
| `CharacterOrigin` | 本体及原始角色 ID，或试用来源；玩法专属角色同样采用模板 |
| `PlayableTraces` | 计入星魂加级后的实际技能等级，以及一次性节点激活状态 |
| `PlayableLightCone` | 光锥种类、叠影及来源；本体通过实例 ID 关联库存 |
| `PlayableRelics` | 本体内外圈配置，或试用六件属性合计 |
| `PlayableRelicGroup` | 一圈原装实例 ID，或映射后的套装及主副词条合计 |
| `CharacterPanel` | 生命、攻击、防御、速度、嘲讽、暴击、命中、抵抗、击破、回能、治疗、增伤与欢愉度 |

角色与光锥统一采用 `PLAYABLE_LEVEL = 80`、`PLAYABLE_PROMOTION = 6`。试用与默认映射光锥叠影采用共用常量 1。

生命、攻击、防御、速度使用 `PanelStat { base, added }`。前三项的基础值为角色与光锥基础值之和；速度的基础值为角色基础速度加适用的基础速度增加量，包含光锥的 `BaseSpeed` 属性。`added = base × 百分比加成合计 + 固定加成合计`，总值由 `PanelStat::total()` 计算。百分比使用比例值，例如 `0.08` 表示 8%。能量恢复效率以 `1.0` 为基础值。

静态属性计入已激活小行迹、遗器主副词条、套装无条件常驻属性和命途匹配的光锥无条件常驻属性。动态效果由战斗引擎计算，组装时按这一约定衔接各来源，保证静态贡献只计入一次。换装后重新计算静态面板。

持有库存保存可分配的实际实例。角色引用库存装备；试用和映射遗器保存原表提供的合计配置。不同账号分别导入和转换，包括用于全试用挑战的账号。

## 计算资料

`PlayableData::bundled()` 读取以下运行数据：

| 文件 | 内容 |
| --- | --- |
| `s2-v4.7/characters.json` | 61 个角色的身份、命途、稀有度及静态基础属性 |
| `s2-v4.7/light-cones.json` | 172 个光锥的命途、基础属性和各叠影常驻属性 |
| `s2-v4.7/trial-templates.json` | 61 份试用星魂、基础行迹、光锥与六件遗器配置 |
| `s2-v4.7/mapping-templates.json` | 60 个常规角色的内外圈映射配置 |
| `s2-v4.7/default-light-cones.json` | 各命途默认光锥 |
| `s2-v4.7/minor-traces.json` | 60 个角色的 201～210 小行迹属性 |
| `s2-v4.7/mainstats.json` | 五星 +15 遗器主词条数值 |
| `s2-v4.7/set-properties.json` | 达到指定件数时生效的套装常驻属性 |
| `s2-v4.7/manifest.json` | 游戏版本、赛季和资料快照 ID |
| `reliquary-traces-v4.json` | 导入与转换共用的节点、实际技能 ID 和星魂加级关系 |

数值口径见[角色基础数据](../../game-data/character-data.md)、[光锥基础数据](../../game-data/light-cone-data.md)、[试用与映射](../../game-data/character-trial-mapping.md)及[小行迹表](../../game-data/currency-wars-minor-traces.md)。节点与常驻属性采用 [TurnBasedGameData 4.7 固定配置](https://github.com/DimbreathBot/TurnBasedGameData/tree/dfdfe3337bcfa14f000074d2dae7058118193ea6/ExcelOutput)。局内形态、星级、强度和伤害修正的共用数值见[货币战争角色数据](../../game-data/currency-wars-character-data.md)，由后续模块使用。

## 转换规则

入口先检查账号版本、重复 ID、装备引用、部位和跨角色占用，再按试用表顺序生成全部角色配置。持有角色采用本体，缺席角色补齐试用，玩法专属角色从模板取得。

角色 ID 对照由调用方显式提供。例如已确认的开拓者 `8010 → 8009` 会保留原始 ID；多个持有条目对应同一目标时返回冲突。

| 部分 | 转换规则 |
| --- | --- |
| 星魂 | 本体取实际星魂与保底的较大值：四星下限 6、五星下限 0；试用采用试用星魂 |
| 行迹 | 选择最终能力版本，转移本体进度，基础等级逐项补到试用下限，再按实际技能 ID 加星魂等级一次 |
| 激活节点 | 补齐试用已激活节点，保留本体已激活节点；所需未知状态返回具体缺项 |
| 光锥 | 本体保留种类和叠影，空装采用命途默认光锥；试用采用模板光锥 |
| 遗器 | 内外圈分别判断，任意一件低于 +15 或部位为空就整圈映射；全部 +15 时保留原装 |

例如外圈头部 +12、其余三件 +15，外圈采用整圈映射套装及属性，内圈独立判断。试用表提供六件属性合计，映射表提供内外圈各自合计，两者都已包含主副词条，套装常驻属性另算。每份遗器属性计入一次，转换保持原始账号不变。

## 运行与验证

在仓库根目录运行，输出目录需预先存在：

```powershell
cargo run --locked -p account-core --example convert_playable -- <导出.json> <静态配置.json> 8010=8009
cargo test --locked -p account-core
cargo fmt --all -- --check
cargo clippy --locked -p account-core --all-targets -- -D warnings
```

测试直接检查完整转换结果，覆盖本体与试用来源、逐技能星魂加级、静态面板、角色 ID 对照、每个遗器部位的缺件或等级不足、内外圈独立映射及资源引用冲突。
