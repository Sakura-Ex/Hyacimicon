# Rust 运营后端

本目录是货币战争运营模拟的 Rust 后端，提供商店、经验、投资、节点、固定阵容格位和 HTTP 会话接口。网页静态文件复用 `research/operations/web/`，图鉴数据从 `hsr-currency-wars-data` 读取。

## 运行

在仓库根目录执行：

```powershell
cargo run -p hsr-operations-backend
```

默认监听 `127.0.0.1:8765` 并打开浏览器。关闭自动打开：

```powershell
cargo run -p hsr-operations-backend -- --no-open
```

指定数据仓库：

```powershell
$env:HSR_CURRENCY_WARS_DATA = "E:\hsr-ai\hsr-currency-wars-data"
cargo run -p hsr-operations-backend
```

## 开局规则

每局从池中随机选择一种开局模板：

- 1 个三费角色和 3 个一费角色。
- 2 个二费角色和 2 个一费角色。

开局金币为 3。会替换初始角色的旧投资环境已停用，避免破坏固定开局模板；其他不改变初始阵容的投资环境继续生效。

## 验证

```powershell
cargo test -p hsr-operations-backend
cargo fmt --check
```
