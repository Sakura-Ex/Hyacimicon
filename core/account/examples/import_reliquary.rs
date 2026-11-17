//! 从本地导出文件导入账号，并将统一配置与报告写入指定目录。

use std::{env, error::Error, fs, io, path::PathBuf};

use account_core::import::reliquary::import_reliquary_archive;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "用法：import_reliquary <导出 JSON 路径> <输出目录>",
        )
        .into());
    }
    let input = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    let config_id = input
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "输入文件名应包含有效配置标识")
        })?;
    let imported = import_reliquary_archive(&fs::read_to_string(input.as_path())?, config_id)?;
    fs::create_dir_all(&output)?;
    fs::write(
        output.join("account.json"),
        serde_json::to_vec_pretty(&imported.account)?,
    )?;
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&imported.report)?,
    )?;
    let equipped_cones = imported
        .account
        .characters
        .iter()
        .filter(|character| character.light_cone_instance_id.is_some())
        .count();
    let equipped_relics: usize = imported
        .account
        .characters
        .iter()
        .map(|character| character.relic_instance_ids.len())
        .sum();
    println!(
        "来源 {}，构建 {}，格式版本 {}",
        imported.report.source, imported.report.build, imported.report.format_version
    );
    println!(
        "{} 个角色，{} 件光锥（{} 件闲置），{} 件遗器（{} 件闲置）",
        imported.account.characters.len(),
        imported.account.light_cones.len(),
        imported.account.light_cones.len() - equipped_cones,
        imported.account.relics.len(),
        imported.account.relics.len() - equipped_relics
    );
    println!(
        "行迹绑定 {}，{} 项待补齐",
        imported.report.trace_bindings_snapshot,
        imported.report.warnings.len()
    );
    for warning in &imported.report.warnings {
        println!("{}：{}", warning.path, warning.message);
    }
    println!("已写入 {}", output.display());
    Ok(())
}
