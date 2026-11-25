# 运行数据与开发环境

主仓库维护 Rust 数据结构、JSON 解析、算法、接口及合成测试。私有 `Hyacimicon-data` 固定经过整理的规则资料及运行快照，原始采集素材和清洗工具仍由原数据仓库维护。

## 初始化研究数据

具有私有仓库访问权限的合作者在主仓库根目录执行：

```powershell
git submodule sync -- docs/game-data
git submodule update --init -- docs/game-data
$env:HYACIMICON_DATA_DIR = (Resolve-Path docs/game-data/runtime).Path
```

运行目录结构：

```text
runtime/
├── account/
│   ├── reliquary-traces-v4.json
│   └── s2-v4.7/
└── account-generation/
```

调用者明确选择快照目录及行迹 JSON；加载器一次解析为内存中的 Rust 数据结构。账号导入、静态配置和生成器共用同一份选定的行迹资料。游戏版本变化时更新配套数据，记录 Git 提交与清单标识。

## 构建与测试

公开贡献者无需初始化私有子模块即可编译和运行公开测试：

```powershell
cargo test --workspace --locked
```

公开测试使用自建样例，检查文件加载、错误路径、数据转换及数据无关的算法。拥有数据权限时，在设置上述环境变量后额外运行所有历史快照回归：

```powershell
cargo test --workspace --locked --features account-generator/private-data-tests
```

该 feature 仅启用依赖真实快照的测试；资料仍在运行时读取。缺少或无权限访问资料时，私有回归明确失败。发布产物无需嵌入这些 JSON。

## 更新

先更新并提交私有数据仓库对应快照，再更新主仓库固定引用。新版本可以放在独立目录；当前工具示例使用 `s2-v4.7`。运行 JSON 不提交到公开代码仓库。
