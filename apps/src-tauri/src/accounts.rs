use std::{collections::BTreeSet, path::Path, sync::OnceLock};

use account_core::{
    import::reliquary::{import_reliquary_archive, ImportReport},
    raw::validate_raw_account,
    reference::TraceCatalog,
    RawAccount, RelicSlot,
};
#[cfg(feature = "research")]
use account_generator::{
    generate_account, AccountReference, AccountRequest, ChaCha8Rng, SeedableRng, GENERATOR_VERSION,
};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::account_storage::{AccountFiles, Origin, StoredAccount};

const CHARACTERS: &str = include_str!("../character-names.tsv");
const CONES: &str = include_str!("../light-cone-names.tsv");
const RELICS: &str = include_str!("../relic-set-names.tsv");

#[derive(Default)]
pub struct AccountStore {
    data: OnceLock<Result<TraceCatalog, String>>,
    #[cfg(feature = "research")]
    generator: OnceLock<Result<AccountReference, String>>,
}

fn account_files() -> Result<AccountFiles, String> {
    let exe = std::env::current_exe().map_err(|error| format!("无法定位程序文件：{error}"))?;
    let directory = exe
        .parent()
        .ok_or("无法定位程序目录")?
        .join(if cfg!(feature = "research") {
            "accounts"
        } else {
            "player-accounts"
        });
    Ok(AccountFiles::new(directory))
}

impl AccountStore {
    fn data(&self) -> Result<&TraceCatalog, String> {
        self.data
            .get_or_init(|| {
                TraceCatalog::bundled().map_err(|error| format!("读取账号资料失败：{error}"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    #[cfg(feature = "research")]
    fn generator(&self) -> Result<&AccountReference, String> {
        self.generator
            .get_or_init(|| {
                AccountReference::bundled().map_err(|error| format!("读取生成器资料失败：{error}"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn save(&self, entry: StoredAccount) -> Result<Value, String> {
        let result = self.display(&entry)?;
        account_files()?.save(&entry)?;
        Ok(result)
    }

    fn display(&self, entry: &StoredAccount) -> Result<Value, String> {
        Ok(display_account(entry, self.data()?))
    }
}

fn parse_import(json: &str, config_id: &str) -> Result<(RawAccount, Option<ImportReport>), String> {
    #[derive(serde::Deserialize)]
    struct Format {
        schema_version: Option<u32>,
    }
    let input = json.trim_start_matches('\u{feff}');
    let format: Format =
        serde_json::from_str(input).map_err(|error| format!("账号 JSON 解析失败：{error}"))?;
    if format.schema_version.is_some() {
        let raw = serde_json::from_str(input)
            .map_err(|error| format!("统一账号 JSON 不符合格式：{error}"))?;
        validate_raw_account(&raw).map_err(|error| error.to_string())?;
        Ok((raw, None))
    } else {
        let imported =
            import_reliquary_archive(input, config_id).map_err(|error| error.to_string())?;
        Ok((imported.account, Some(imported.report)))
    }
}

fn source_filename(file_name: &str) -> &str {
    Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("account.json")
}

fn import_name(file_name: &str) -> &str {
    Path::new(file_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("导入账号")
}

fn validated_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 100 || trimmed.chars().any(char::is_control)
    {
        return Err("账号名称应包含 1 至 100 个字符，且不能包含控制符。".into());
    }
    Ok(trimmed.to_owned())
}

fn account_summary(raw: &RawAccount) -> Value {
    let equipped_cones: BTreeSet<_> = raw
        .characters
        .iter()
        .filter_map(|character| character.light_cone_instance_id.as_deref())
        .collect();
    let equipped_relics: BTreeSet<_> = raw
        .characters
        .iter()
        .flat_map(|character| character.relic_instance_ids.values().map(String::as_str))
        .collect();
    json!({
        "characters": raw.characters.len(),
        "lightCones": raw.light_cones.len(),
        "relics": raw.relics.len(),
        "idleLightCones": raw.light_cones.iter().filter(|item| !equipped_cones.contains(item.instance_id.as_str())).count(),
        "idleRelics": raw.relics.iter().filter(|item| !equipped_relics.contains(item.instance_id.as_str())).count(),
    })
}

fn origin_summary(origin: &Origin) -> Value {
    match origin {
        Origin::Imported {
            filename, report, ..
        } => json!({
            "kind": "imported", "filename": filename, "report": report,
        }),
        Origin::Generated {
            seed,
            request,
            report,
            generator_version,
        } => json!({
            "kind": "generated", "seed": seed, "request": request, "report": report, "generatorVersion": generator_version,
        }),
    }
}

#[tauri::command]
pub async fn player_accounts(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (entries, warnings) = account_files()?.list()?;
        let store = app.state::<AccountStore>();
        let accounts = entries
            .iter()
            .map(|entry| store.display(entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({ "accounts": accounts, "warnings": warnings }))
    })
    .await
    .map_err(|error| format!("读取账号任务失败：{error}"))?
}

#[tauri::command]
pub async fn player_inspect_account(file_name: String, json: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (raw, report) = parse_import(&json, "inspection")?;
        let name: String = import_name(&file_name).chars().take(100).collect();
        Ok(json!({ "name": name, "summary": account_summary(&raw), "report": report }))
    })
    .await
    .map_err(|error| format!("检查导入文件失败：{error}"))?
}

#[tauri::command]
pub async fn player_import_account(
    app: AppHandle,
    file_name: String,
    json: String,
    name: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = validated_name(&name)?;
        let id = Uuid::new_v4();
        let (raw, report) = parse_import(&json, &format!("import-{id}"))?;
        app.state::<AccountStore>().save(StoredAccount::new(
            id,
            name,
            raw,
            Origin::Imported {
                filename: source_filename(&file_name).into(),
                original_json: json,
                report,
            },
        ))
    })
    .await
    .map_err(|error| format!("导入任务失败：{error}"))?
}

#[cfg(feature = "research")]
#[tauri::command]
pub async fn player_generate_account(
    app: AppHandle,
    request: AccountRequest,
    seed: Option<String>,
    name: Option<String>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let id = Uuid::new_v4();
        let name = name.as_deref().map(validated_name).transpose()?;
        let seed = match seed {
            Some(seed) => seed
                .parse::<u64>()
                .map_err(|_| "种子应为 0 至 18446744073709551615 的整数。")?,
            None => id.as_u128() as u64,
        };
        let store = app.state::<AccountStore>();
        let generated = generate_account(
            &request,
            store.generator()?,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .map_err(|error| format!("生成账号失败：{error}"))?;
        let report = json!({
            "goldCount": generated.gold_count(),
            "meanEffectiveRolls": generated.mean_effective_rolls(),
            "coreCharacterIds": generated.core_character_ids,
            "coreGoldShare": generated.core_gold_share(),
            "coreRelicShare": generated.core_relic_share(),
            "goldByCategory": generated.gold_by_category(),
            "copiesByCategory": generated.copies_by_category(),
            "investments": generated.investments,
            "attempts": generated.attempts,
        });
        let config =
            serde_json::to_value(&request).map_err(|error| format!("保存生成参数失败：{error}"))?;
        let name = name.unwrap_or_else(|| format!("生成账号 {}", &id.simple().to_string()[..8]));
        store.save(StoredAccount::new(
            id,
            name,
            generated.account,
            Origin::Generated {
                request: config,
                seed: seed.to_string(),
                generator_version: GENERATOR_VERSION.into(),
                report,
            },
        ))
    })
    .await
    .map_err(|error| format!("生成任务失败：{error}"))?
}

#[cfg(feature = "research")]
#[tauri::command]
pub async fn player_account_catalogue(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<AccountStore>();
        let reference = store.generator()?;
        let display_ids = |ids: Vec<&str>, catalogue: &str| -> Vec<Value> {
            ids.into_iter()
                .map(|id| json!({ "id": id, "name": catalogue_name(catalogue, id).unwrap_or(id) }))
                .collect()
        };
        Ok(json!({
            "characters": display_ids(reference.character_ids().collect(), CHARACTERS),
            "lightCones": display_ids(reference.light_cone_ids().collect(), CONES),
        }))
    })
    .await
    .map_err(|error| format!("读取生成器候选资料失败：{error}"))?
}

#[tauri::command]
pub fn player_rename_account(app: AppHandle, id: Uuid, name: String) -> Result<Value, String> {
    let name = validated_name(&name)?;
    let mut entry = account_files()?.load(id)?;
    entry.name = name;
    app.state::<AccountStore>().save(entry)
}

#[tauri::command]
pub fn player_delete_account(id: Uuid) -> Result<(), String> {
    account_files()?.remove(id)
}

#[tauri::command]
pub fn player_export_account(id: Uuid, kind: String) -> Result<String, String> {
    let files = account_files()?;
    let entry = files.load(id)?;
    let (suffix, content) = match (kind.as_str(), entry.origin) {
        ("account", _) => ("account", serde_json::to_string_pretty(&entry.raw)),
        (
            "original",
            Origin::Imported {
                original_json,
                report: Some(_),
                ..
            },
        ) => ("fribbels", Ok(original_json)),
        ("original", Origin::Imported { original_json, .. }) => ("source", Ok(original_json)),
        (
            "report",
            Origin::Imported {
                report: Some(report),
                ..
            },
        ) => ("report", serde_json::to_string_pretty(&report)),
        ("report", Origin::Imported { .. }) => (
            "report",
            serde_json::to_string_pretty(&json!({
                "source": "raw_account", "schemaVersion": entry.raw.schema_version,
                "summary": account_summary(&entry.raw),
            })),
        ),
        ("report", Origin::Generated { report, .. }) => {
            ("report", serde_json::to_string_pretty(&report))
        }
        (
            "request",
            Origin::Generated {
                request,
                seed,
                generator_version,
                ..
            },
        ) => (
            "request",
            serde_json::to_string_pretty(&json!({
                "request": request, "seed": seed, "generatorVersion": generator_version,
                "gameVersion": entry.raw.game_version, "dataSnapshot": entry.raw.data_snapshot,
            })),
        ),
        _ => return Err("该账号没有所选文件。".into()),
    };
    let content = content.map_err(|error| format!("编码导出文件失败：{error}"))?;
    Ok(files
        .export(id, suffix, &content)?
        .to_string_lossy()
        .into_owned())
}

fn catalogue_name<'a>(catalogue: &'a str, id: &str) -> Option<&'a str> {
    catalogue.lines().find_map(|line| {
        let mut fields = line.split('\t');
        let key = fields.next()?;
        let name = fields.next()?;
        (key == id).then_some(name)
    })
}

fn catalogue_path(id: &str) -> Option<&'static str> {
    CHARACTERS.lines().find_map(|line| {
        let mut fields = line.split('\t');
        if fields.next()? != id {
            return None;
        }
        fields.next()?;
        fields.next()
    })
}

fn relic_art_key(slot: RelicSlot) -> (&'static str, u8) {
    match slot {
        RelicSlot::Head => ("头部", 0),
        RelicSlot::Hands => ("手部", 1),
        RelicSlot::Body => ("躯干", 2),
        RelicSlot::Feet => ("脚部", 3),
        RelicSlot::PlanarSphere => ("位面球", 0),
        RelicSlot::LinkRope => ("连结绳", 1),
    }
}

fn display_account(entry: &StoredAccount, data: &TraceCatalog) -> Value {
    let raw = &entry.raw;
    let artwork = |path: String| {
        let safe = path[1..].split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        });
        safe.then_some(path)
    };
    let characters: Vec<_> = raw.characters.iter().map(|character| {
        let id = &character.character_id;

        let binding = character.ability_version.and_then(|version| data.binding(id, version));
        let mut skills = Vec::new();
        if let Some(binding) = binding {
            for (key, tab) in [("basic", "普攻"), ("skill", "战技"), ("ult", "终结技"),
                ("talent", "天赋"), ("technique", "秘技"), ("memosprite.skill", "忆灵技"),
                ("memosprite.talent", "忆灵天赋"), ("aha_biography", "欢愉传记"), ("elation", "欢愉技")] {
                if let Some(node) = binding.skills.get(key) {
                    skills.push(json!({ "group": "角色技能", "tab": tab, "title": null,
                        "level": character.traces.skill_levels.get(&node.node_id),
                        "description": "", "scaling": [] }));
                }
            }
            for (key, node) in &binding.nodes {
                if let Some(unlocked) = character.traces.node_unlocks.get(node) {
                    skills.push(json!({ "group": "行迹节点", "tab": key, "title": null,
                        "level": null, "description": if *unlocked { "已解锁" } else { "未解锁" }, "scaling": [] }));
                }
            }
        }
        let cone = character.light_cone_instance_id.as_ref()
            .and_then(|id| raw.light_cones.iter().find(|cone| &cone.instance_id == id))
            .map(|cone| json!({
                "name": catalogue_name(CONES, &cone.light_cone_id).map(str::to_owned)
                    .unwrap_or_else(|| format!("光锥 {}", cone.light_cone_id)),
                "image": artwork(format!("/light-cones/{}.png", cone.light_cone_id)).unwrap_or_default(),
                "detail": format!("Lv. {} · 突破 {} · 叠影 {}", cone.level, cone.promotion, cone.superimposition),
                "count": 1,
            }));
        let relics: Vec<_> = character.relic_instance_ids.iter().filter_map(|(slot, id)| {
            let relic = raw.relics.iter().find(|relic| &relic.instance_id == id)?;
            let (slot_name, art_index) = relic_art_key(*slot);
            Some(json!({
                "name": slot_name,
                "image": artwork(format!("/relics/{}_{art_index}.png", relic.set_id)).unwrap_or_default(),
                "detail": format!("{}★ +{} · {}", relic.rarity, relic.level,
                    catalogue_name(RELICS, &relic.set_id).unwrap_or(&relic.set_id)),
                "count": 1,
            }))
        }).collect();
        let mut view = json!({
            "id": id,
            "name": catalogue_name(CHARACTERS, id).map(str::to_owned).unwrap_or_else(|| format!("角色 {id}")),
            "image": artwork(format!("/avatars/head-{id}.png")).unwrap_or_default(),
            "path": catalogue_path(id).unwrap_or_default(),
            "level": character.level, "ascension": character.promotion, "eidolons": character.eidolons,
            "stats": [], "skills": skills, "relics": relics,
        });
        if let Some(portrait) = artwork(format!("/portraits/{id}.png")) { view["portrait"] = json!(portrait); }
        if let Some(cone) = cone { view["cone"] = cone; }
        view
    }).collect();
    json!({ "id": entry.id, "name": entry.name, "source": entry.source(), "characters": characters, "summary": account_summary(raw), "origin": origin_summary(&entry.origin) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_export_can_be_reimported_and_invalid_inventory_is_rejected() {
        let input = include_str!("../../../core/account/tests/fixtures/raw_account.json");
        let expected: RawAccount = serde_json::from_str(input).expect("raw fixture");
        let (mut raw, report) = parse_import(input, "inspection").expect("reimport raw account");
        assert_eq!(raw, expected);
        assert!(report.is_none());
        raw.characters[0].light_cone_instance_id = Some("missing-instance".into());
        let wrong = serde_json::to_string(&raw).expect("encode invalid account");
        let error = parse_import(&wrong, "inspection").expect_err("decline missing reference");
        assert!(error.contains("missing-instance") && error.contains("不存在"));
    }

    #[test]
    fn fribbels_fixture_retains_roster_and_loadout() {
        let imported = import_reliquary_archive(
            include_str!("../../../core/account/tests/fixtures/reliquary_v4.json"),
            "fixture",
        )
        .expect("import fixture");
        let id = Uuid::new_v4();
        let entry = StoredAccount::new(
            id,
            "fixture".into(),
            imported.account,
            Origin::Imported {
                filename: "fixture.json".into(),
                original_json: String::new(),
                report: Some(imported.report),
            },
        );
        let view = display_account(&entry, &TraceCatalog::bundled().expect("catalogue"));
        assert_eq!(view["id"], id.to_string());
        let characters = view["characters"].as_array().expect("display roster");
        assert_eq!(characters.len(), 3);
        assert_eq!(characters[0]["id"], "1004");
        assert_eq!(characters[0]["name"], "瓦尔特");
        assert_eq!(characters[0]["level"], 60);
        assert_eq!(characters[0]["skills"][0]["level"], 3);
        assert_eq!(characters[0]["relics"][0]["name"], "脚部");
        assert_eq!(characters[0]["cone"]["count"], 1);
        assert!(characters[1]["skills"]
            .as_array()
            .expect("trace list")
            .iter()
            .any(|skill| skill["tab"] == "忆灵技" && skill["level"] == 6));
    }
}
