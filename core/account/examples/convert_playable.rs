//! 从 reliquary-archiver 文件生成完整游玩配置。

use std::{collections::BTreeMap, env, error::Error, fs, io};

use account_core::{
    import::reliquary::import_reliquary_archive,
    playable::conversion::{ConversionRequest, convert_playable_account},
    reference::PlayableData,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    if arguments.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "用法：convert_playable <导出文件> <输出文件> [原始角色ID=模板角色ID ...]",
        )
        .into());
    }
    let mut aliases = BTreeMap::new();
    for argument in &arguments[2..] {
        if let Some((source, target)) = argument.split_once('=') {
            if aliases
                .insert(source.to_owned(), target.to_owned())
                .is_some()
            {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "角色 ID 对照重复").into());
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("未知参数 {argument}"),
            )
            .into());
        }
    }
    let imported = import_reliquary_archive(&fs::read_to_string(&arguments[0])?, "player-account")?;
    let data = PlayableData::bundled()?;
    let result = convert_playable_account(
        &imported.account,
        &data,
        &ConversionRequest {
            config_id: "player-account-s2".to_owned(),
            character_aliases: aliases,
        },
    )?;
    fs::write(&arguments[1], serde_json::to_vec_pretty(&result)?)?;
    println!("已生成 {} 个角色的完整游玩配置", result.characters.len());
    Ok(())
}
