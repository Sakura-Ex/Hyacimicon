# Rust 原始账号模型

状态：统一数据模型与 Serde 序列化已实现。玩家导入、Python 生成结果接入、玩法配置转换和 Python 绑定待实现。

`core/account/` 提供 `account-core` crate，供训练、评估和玩家应用共用。当前接收项目统一格式；来源适配器后续负责将 reliquary-archiver 等输入转换为该格式。整体流程见[账号数据流](../account-generation/data-flow.md)。

## 目录与入口

```text
core/account/
├── Cargo.toml
├── src/
│   ├── lib.rs          # 公开类型
│   ├── raw.rs          # 账号标识、版本与完整库存
│   ├── character.rs    # 角色、行迹与装备引用
│   ├── equipment.rs    # 光锥、遗器实例和部位
│   └── stats.rs        # 统一主副词条键
├── examples/raw_account.rs
└── tests/
    ├── raw_account.rs
    └── fixtures/raw_account.json
```

所有 Rust crate 在根 `Cargo.toml` 中组织为 workspace，当前成员为 `core/account`。后续训练绑定放在 `research/sim_native/`，通过路径依赖使用账号核心。

## 统一配置

| 类型 | 保存内容 |
| --- | --- |
| `RawAccount` | 内部格式版本、配置 ID、可选游戏版本与快照、角色列表、光锥和遗器完整库存 |
| `CharacterConfig` | 常规角色 ID、原始等级、突破、星魂、可选能力版本、行迹和装备引用 |
| `TraceConfig` | 真实行迹节点 ID 对应的基础等级，以及一次性节点的解锁状态 |
| `LightConeInstance` | 实例 ID、光锥种类 ID、原始等级、突破和叠影 |
| `RelicInstance` | 实例 ID、套装、部位、实际品质、强化等级、主词条与当前副词条 |
| `RelicSlot` | 头、手、躯干、脚、球、绳，序列化为数字 1～6 |
| `RelicMainStat`、`RelicSubStat` | 与现有 Python 生成器一致的属性键 |

`schema_version` 当前为 `RAW_ACCOUNT_SCHEMA_VERSION = 1`，表示项目统一格式。来源的导出格式版本由后续适配器单独记录。`config_id` 由调用方分配；游戏版本和快照未知时保存 `None`，序列化为 `null`。

角色和库存使用列表，保留来源顺序和重复条目，供后续接入接口报告重复 ID。每件装备使用实例 ID，光锥种类 ID 可以重复。装备归属集中在角色上维护：光锥引用为 `None` 表示空装，遗器按部位映射，缺少的部位表示未装备；库存中未被引用的装备保持闲置。

角色形态按常规角色 ID 分别保存。多形态之间的共享资源关系和使用限制在后续配置转换中处理。

## 行迹与单位

`TraceConfig.skill_levels` 使用真实行迹节点 ID，记录星魂加级前的基础等级。普攻、战技、终结技、天赋、秘技、忆灵技能、欢愉技等按资料关联到相应节点。`node_unlocks` 使用节点 ID 到布尔值的映射，明确区分已解锁、未解锁与信息未知：

- `true` 表示已解锁，`false` 表示未解锁。
- 缺少某个节点键表示该节点的信息未知。
- 所需节点是否齐全，由接入接口结合资料快照检查。

`ability_version` 保留来源提供的能力配置版本标识；`Some(0)` 与未知的 `None` 分别表达。具体机制由对应版本资料解释。

百分比副词条使用比例值，例如 `0.0324` 表示 3.24%；固定生命、攻击、防御和速度使用实际值，保留小数精度。主词条保存已经区分固定值与百分比的键，最终数值在配置转换中按品质、强化等级和快照查询。属性键约定见[遗器数据](../../game-data/relic-data.md#字段与单位)。

来源适配器需先完成主词条歧义处理、百分比换算和行迹绑定。尚未确认技能等级是否包含星魂加级时，保留来源记录，返回具体缺项，待口径确认后形成统一行迹数据。锁定、弃置、预览词条等来源字段由适配器另行保存；当前副词条由 `substats` 表达。

## 接入与转换

本阶段的数据类型表达字段与单位。Serde 检查字段类型、已知枚举和必需字段；结构体拒绝未知字段，帮助发现尚未适配的来源字段。字段有值时按实际内容保存。

引用完整性、重复占用、数值范围、有限数值、行迹完整性、资料匹配和格式版本兼容性检查待接入接口实现。直接构造或反序列化成功表示已取得该数据结构，后续转换入口仍需完成这些检查。

导入报告在原始账号之外保存错误、缺失字段和未知编码。生成器继续在自己的结果中保存生成参数、随机种子、投入评分与搜索统计，并在接入时提供 `RawAccount`。试用补齐、练度保底、星魂附加等级与生效属性计算由后续配置转换完成。

## 运行与验证

安装 Rust 工具链后，在仓库根目录运行：

```powershell
cargo run --locked -p account-core --example raw_account
cargo test --locked -p account-core
cargo fmt --all -- --check
cargo clippy --locked -p account-core --all-targets -- -D warnings
```

样例为人工构造的统一格式数据，覆盖空装、部分配装、闲置装备、同种光锥多个实例、多形态、不同遗器品质和强化等级，以及未知行迹信息。数值用于验证模型表达和序列化；规则合法性与 reliquary-archiver 实际文件导入将在相应功能接入时验证。
