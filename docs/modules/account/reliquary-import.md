# reliquary-archiver 账号导入

状态：Rust 导入已实现，使用 build `0.19.0`、格式版本 `4` 的真实导出文件完成验证。导入结果供后续实际游玩配置转换使用。

## 使用方式

在仓库根目录运行：

```powershell
cargo run --locked -p account-core --example import_reliquary -- "local-data/archive.json" "local-data/account-import"
```

第一个参数是来源文件，第二个是输出目录。示例用来源文件名分配配置 ID，向目录写入 `account.json` 和 `report.json`；再次使用相同目录时更新这两个文件。终端显示角色与库存数量、闲置数量和待补齐项。真实玩家数据及输出保存在已忽略的本地目录。

库接口为：

```rust
use account_core::import::reliquary::import_reliquary_archive;

let imported = import_reliquary_archive(&source_json, "account-config-001")?;
```

`source_json` 为已读取的来源 JSON 字符串，配置 ID 由调用方分配。返回 `ReliquaryImport`：

| 字段 | 内容 |
| --- | --- |
| `account` | 统一的 `RawAccount`，含原始练度、真实行迹节点和完整库存 |
| `report` | 导出器版本、来源格式版本、行迹绑定依据及缺失或未知字段 |

示例向磁盘写入统一账号与报告。名称、锁定、弃置、预览和重掷词条、材料等非计算字段保留在原始导出文件中，导入结果仅保存参与计算的字段和提示。

## 字段转换

| 来源 | 统一配置 |
| --- | --- |
| 角色 `id / level / ascension / eidolon` | `character_id / level / promotion / eidolons` |
| `ability_version` | 保留为已知能力版本，包括 `Some(0)` |
| 光锥、遗器 `_uid` | 配置 ID、装备类别和来源实例 ID 组成的命名空间标识 |
| 装备 `location` | 角色上的装备实例引用；空字符串表示闲置 |
| 英文遗器部位 | 部位枚举，统一序列化为 1～6 |
| 主词条 `HP / ATK / DEF` | 结合部位区分固定值与百分比，得到 `HP / ATK / HP_ / ATK_ / DEF_` |
| 百分比副词条 | 来源数值除以 100，例如 `2.9160001` 转为 `0.029160001` |
| 固定副词条、速度 | 保留原数值与小数精度 |
| `skills / memosprite / traces` | 按角色 ID、能力版本和固定资料绑定到真实行迹节点 ID |

完整库存和各角色形态按来源顺序保留。当前遗器属性采用 `substats` 的最终数值；来源的获得次数、档位统计、预览和重掷内容由原始文件保留。

来源文件未声明游戏版本与完整游戏数据快照，统一配置中的 `game_version` 和 `data_snapshot` 保持未知。行迹映射使用的资料单独记录在 `report.trace_bindings_snapshot`，后续转换再关联完整游戏资料。

## 行迹依据与资料版本

格式语义核对了 [reliquary-archiver v0.19.0 的转换实现](https://github.com/IceDynamix/reliquary-archiver/blob/24e2035b16f94799cb1c349e27edc6fe7d7ca897/src/export/fribbels/converters.rs)、[来源结构](https://github.com/IceDynamix/reliquary-archiver/blob/24e2035b16f94799cb1c349e27edc6fe7d7ca897/src/export/fribbels/models.rs)和[属性标签转换](https://github.com/IceDynamix/reliquary-archiver/blob/24e2035b16f94799cb1c349e27edc6fe7d7ca897/src/export/fribbels/utils.rs)。对应提交为 `24e2035b16f94799cb1c349e27edc6fe7d7ca897`。本模块按字段含义独立实现来源适配。

技能等级由导出器直接从账号技能树等级写出，导出时没有叠加星魂等级。`ability_version` 来自技能树版本，增强角色按该版本绑定到对应节点 ID。

随 crate 提供的最小绑定资料为 `core/account/data/reliquary-traces-v4.json`：

- 快照 ID：`reliquary-traces-4.7-dfdfe3337bcf`；条目类别：`character_trace_binding`；条目 ID：`角色 ID:能力版本`。
- 游戏资料版本：4.7；来源提交：`dfdfe3337bcfa14f000074d2dae7058118193ea6`。
- 节点来源：[AvatarSkillTreeConfig.json](https://github.com/DimbreathBot/TurnBasedGameData/blob/dfdfe3337bcfa14f000074d2dae7058118193ea6/ExcelOutput/AvatarSkillTreeConfig.json) 与 [AvatarSkillTreeConfigLD.json](https://github.com/DimbreathBot/TurnBasedGameData/blob/dfdfe3337bcfa14f000074d2dae7058118193ea6/ExcelOutput/AvatarSkillTreeConfigLD.json)。转换共用该资料的实际技能 ID，以及 `AvatarRankConfig.json`／`AvatarRankConfigLD.json` 中的星魂加级列表。
- 按一级记录的 `AvatarID + EnhancedID + AnchorType` 提取真实节点 ID 与等级上限，得到 99 种角色形态、109 个角色与能力版本组合。技能节点保存等级上限和技能 ID，一次性节点直接保存标签到节点 ID 的对应关系。

来源标签 `basic/skill/ult/talent` 分别对应 `Point01`～`Point04`，`ability_1`～`ability_3` 对应 `Point06`～`Point08`，`stat_1`～`stat_10` 对应 `Point09`～`Point18`；忆灵技能对应 `Point19/Point20`，`special` 对应 `Point21`，欢愉技对应 `Point22`。

来源省略了秘技等级。资料中 `Point05` 明确为默认解锁、固定一级时，导入器据此记录一级。导出器为无 `special` 节点的角色也写出 `special=false`，导入器跳过该格式占位；存在节点时保留真实布尔状态。

更新绑定资料时，从固定提交的节点与星魂配置提取所需字段，核对角色与版本组合的唯一性、节点标签、默认一级技能条件和技能加级引用，再运行相关回归用例。游戏资料按仓库分工维护，crate 内保留运行所需的精简快照。

## 检查与返回结果

导入器检查来源名称和格式版本、必需字段及类型、重复角色与装备实例、装备归属、同角色同部位冲突、等级突破范围、主副词条标签及当前词条重复。错误通过 `ImportError` 返回字段路径或 JSON 解析位置。

角色或能力版本超出绑定资料时，返回具体缺项。来源缺少某项技能等级或行迹解锁状态时，统一配置保留未知状态，并在报告中列出字段路径。未识别的来源扩展字段只报告路径，提示其含义待适配。

导入保留角色实际练度。账号转换执行试用补齐、练度保底和静态面板计算。局面模拟检查实际阵容的资源使用限制，战斗输入由组装步骤准备。

## 验证结果

人工样例覆盖完整与部分装备、闲置库存、同种光锥多个实例、低星及未强化遗器、百分比与小数速度、增强角色、忆灵和欢愉技、缺失行迹、未知扩展及错误输入。运行 `cargo test --locked -p account-core` 检查模型和来源适配。

2026-11-16 对提供的真实导出完成逐项对账，结果如下：

| 核对对象 | 数量与结果 |
| --- | --- |
| 角色与原始练度 | 57，全部一致 |
| 光锥实例 | 251，全部一致；210 件闲置 |
| 遗器实例 | 2400，全部一致；2180 件闲置 |
| 当前副词条 | 8187，属性和单位转换一致 |
| 技能节点 | 301，与固定版本原始行迹资料及来源等级一致，含 57 个默认一级秘技 |
| 解锁节点 | 742，与来源布尔状态及资料节点一致 |
| 缺失或未知字段 | 本次文件为 0 项 |

装备分布为：34 个角色一光锥六遗器，14 个空装，6 个一光锥零遗器，1 个一光锥四遗器，2 个零光锥六遗器。真实账号及核对输出保存在 `local-data/`，自动测试使用人工最小样例。
