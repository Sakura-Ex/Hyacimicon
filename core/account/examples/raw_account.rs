//! 读取随 crate 提供的统一格式样例，展示原始账号库存数量。

use std::error::Error;

use account_core::RawAccount;

fn main() -> Result<(), Box<dyn Error>> {
    let account: RawAccount =
        serde_json::from_str(include_str!("../tests/fixtures/raw_account.json"))?;
    println!(
        "配置 {}：{} 个角色，{} 件光锥，{} 件遗器",
        account.config_id,
        account.characters.len(),
        account.light_cones.len(),
        account.relics.len(),
    );
    Ok(())
}
