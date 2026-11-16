//! 读取原始账号并生成来源选择计划，输出为待展开的实际游玩配置计划。

use std::{collections::BTreeMap, env, error::Error, fs, io};

use account_core::{
    RawAccount,
    playable::{
        ConfigurationMode,
        planning::{PlanRequest, plan_playable_account},
    },
    reference::PlayableData,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "用法：plan_playable <原始账号 JSON> <计划 JSON 输出路径>",
        )
        .into());
    }
    let raw: RawAccount = serde_json::from_str(&fs::read_to_string(&arguments[0])?)?;
    let data = PlayableData::bundled()?;
    let request = PlanRequest {
        mode: ConfigurationMode::OwnedWithTrials,
        character_aliases: BTreeMap::new(),
    };
    let plan = plan_playable_account(&raw, &data, &request)?;
    fs::write(&arguments[1], serde_json::to_vec_pretty(&plan)?)?;
    println!(
        "快照 {}，已生成 {} 个角色的来源及遗器映射计划",
        plan.data_snapshot,
        plan.characters.len()
    );
    Ok(())
}
