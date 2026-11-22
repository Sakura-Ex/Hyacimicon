use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

use account_core::{
    import::reliquary::import_reliquary_archive,
    reference::{GamePath, PlayableData},
    RawAccount, RelicSlot,
};
use account_generator::{
    generate_account, AccountReference, AccountRequest, ChaCha8Rng, SeedableRng,
};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

const CHARACTERS: &str = include_str!("../../../../docs/game-data/character-data.md");
const CONES: &str = include_str!("../../../../docs/game-data/light-cone-data.md");
const RELICS: &str = include_str!("../../../../docs/game-data/relic-data.md");

struct StoredAccount {
    raw: RawAccount,
    name: String,
    source: String,
}

#[derive(Default)]
pub struct AccountStore {
    entries: Mutex<BTreeMap<String, StoredAccount>>,
    next_import: AtomicU64,
    data: OnceLock<Result<PlayableData, String>>,
    generator: OnceLock<Result<AccountReference, String>>,
}

impl AccountStore {
    fn data(&self) -> Result<&PlayableData, String> {
        self.data
            .get_or_init(|| {
                PlayableData::bundled().map_err(|error| format!("读取账号资料失败：{error}"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn save(&self, entry: StoredAccount, app: &AppHandle) -> Result<Value, String> {
        let result = self.display(&entry, app)?;
        self.entries
            .lock()
            .map_err(|_| "账号列表状态不可用")?
            .insert(entry.raw.config_id.clone(), entry);
        Ok(result)
    }

    fn display(&self, entry: &StoredAccount, app: &AppHandle) -> Result<Value, String> {
        let assets = app.asset_resolver();
        Ok(display_account(entry, self.data()?, |path| {
            assets
                .get(path.to_owned())
                .is_some_and(|asset| asset.mime_type.starts_with("image/"))
        }))
    }
}

#[tauri::command]
pub fn player_accounts(
    app: AppHandle,
    store: State<'_, AccountStore>,
) -> Result<Vec<Value>, String> {
    store
        .entries
        .lock()
        .map_err(|_| "账号列表状态不可用")?
        .values()
        .map(|entry| store.display(entry, &app))
        .collect()
}

#[tauri::command]
pub async fn player_import_account(
    app: AppHandle,
    file_name: String,
    json: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<AccountStore>();
        let index = store.next_import.fetch_add(1, Ordering::Relaxed);
        let id = format!("import-{index}");
        let imported = import_reliquary_archive(json.trim_start_matches('\u{feff}'), &id)
            .map_err(|error| error.to_string())?;
        let name = Path::new(&file_name)
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("导入账号");
        store.save(
            StoredAccount {
                raw: imported.account,
                name: name.to_owned(),
                source: format!("Fribbels · v{}", imported.report.format_version),
            },
            &app,
        )
    })
    .await
    .map_err(|error| format!("导入任务失败：{error}"))?
}

#[tauri::command]
pub async fn player_generate_account(app: AppHandle, seed: u32) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<AccountStore>();
        let reference = store
            .generator
            .get_or_init(|| {
                AccountReference::bundled().map_err(|error| format!("读取生成器资料失败：{error}"))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let request = AccountRequest::new(format!("generated-{seed}"), 12);
        let generated = generate_account(
            &request,
            reference,
            &mut ChaCha8Rng::seed_from_u64(u64::from(seed)),
        )
        .map_err(|error| format!("生成账号失败：{error}"))?;
        store.save(
            StoredAccount {
                raw: generated.account,
                name: format!("练习账号 {seed}"),
                source: "随机生成".to_owned(),
            },
            &app,
        )
    })
    .await
    .map_err(|error| format!("生成任务失败：{error}"))?
}

fn catalogue_name<'a>(catalogue: &'a str, id: &str) -> Option<&'a str> {
    catalogue.lines().find_map(|line| {
        let mut cells = line.strip_prefix("| ")?.split('|');
        let key = cells.next()?.trim().strip_prefix('`')?.strip_suffix('`')?;
        (key == id).then(|| cells.next().map(str::trim)).flatten()
    })
}

fn path_label(path: GamePath) -> &'static str {
    match path {
        GamePath::Warrior => "毁灭",
        GamePath::Rogue => "巡猎",
        GamePath::Mage => "智识",
        GamePath::Shaman => "同谐",
        GamePath::Warlock => "虚无",
        GamePath::Knight => "存护",
        GamePath::Priest => "丰饶",
        GamePath::Memory => "记忆",
        GamePath::Elation => "欢愉",
    }
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

fn display_account(
    entry: &StoredAccount,
    data: &PlayableData,
    has_art: impl Fn(&str) -> bool,
) -> Value {
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
        (safe && has_art(&path)).then_some(path)
    };
    let characters: Vec<_> = raw.characters.iter().map(|character| {
        let id = &character.character_id;
        let reference = data.characters.iter().find(|entry| &entry.id == id);
        let binding = character.ability_version.and_then(|version| data.traces.binding(id, version));
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
            "path": reference.and_then(|entry| entry.path).map(path_label).unwrap_or_default(),
            "level": character.level, "ascension": character.promotion, "eidolons": character.eidolons,
            "stats": [], "skills": skills, "relics": relics,
        });
        if let Some(portrait) = artwork(format!("/portraits/{id}.png")) { view["portrait"] = json!(portrait); }
        if let Some(cone) = cone { view["cone"] = cone; }
        view
    }).collect();
    json!({ "id": raw.config_id, "name": entry.name, "source": entry.source, "characters": characters })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fribbels_fixture_retains_roster_and_loadout() {
        let raw = import_reliquary_archive(
            include_str!("../../../../core/account/tests/fixtures/reliquary_v4.json"),
            "fixture",
        )
        .expect("import fixture")
        .account;
        let entry = StoredAccount {
            raw,
            name: "fixture".into(),
            source: "Fribbels".into(),
        };
        let view = display_account(&entry, &PlayableData::bundled().expect("catalogue"), |_| {
            false
        });
        assert_eq!(view["id"], "fixture");
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
