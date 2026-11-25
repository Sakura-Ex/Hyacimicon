use std::{
    fs::{self, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use account_core::{import::reliquary::ImportReport, RawAccount, RAW_ACCOUNT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const RECORD_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    Imported {
        filename: String,
        original_json: String,
        report: Option<ImportReport>,
    },
    Generated {
        request: Value,
        seed: String,
        generator_version: String,
        report: Value,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredAccount {
    version: u32,
    pub id: Uuid,
    pub name: String,
    pub raw: RawAccount,
    pub origin: Origin,
}

impl StoredAccount {
    pub fn new(id: Uuid, name: String, raw: RawAccount, origin: Origin) -> Self {
        Self {
            version: RECORD_VERSION,
            id,
            name,
            raw,
            origin,
        }
    }

    pub fn source(&self) -> String {
        match &self.origin {
            Origin::Imported {
                report: Some(report),
                ..
            } => format!("Fribbels v{}", report.format_version),
            Origin::Imported { .. } => format!("统一账号 v{}", self.raw.schema_version),
            Origin::Generated { .. } => "随机生成".into(),
        }
    }
}

pub struct AccountFiles {
    directory: PathBuf,
}

impl AccountFiles {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn path(&self, id: Uuid) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }

    pub fn load(&self, id: Uuid) -> Result<StoredAccount, String> {
        read_record(&self.path(id), id)
    }

    pub fn list(&self) -> Result<(Vec<StoredAccount>, Vec<String>), String> {
        let paths = match fs::read_dir(&self.directory) {
            Ok(paths) => paths,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((vec![], vec![])),
            Err(error) => return Err(format!("读取账号目录失败：{error}")),
        };
        let mut accounts = Vec::new();
        let mut warnings = Vec::new();
        for path in paths {
            let path = match path {
                Ok(entry) => entry.path(),
                Err(error) => {
                    warnings.push(format!("读取账号目录条目失败：{error}"));
                    continue;
                }
            };
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| Uuid::parse_str(stem).ok())
            else {
                continue;
            };
            match read_record(&path, id) {
                Ok(account) => accounts.push(account),
                Err(error) => warnings.push(error),
            }
        }
        accounts.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        warnings.sort();
        Ok((accounts, warnings))
    }

    pub fn save(&self, account: &StoredAccount) -> Result<(), String> {
        write_atomic(&self.path(account.id), |writer| {
            serde_json::to_writer(writer, account).map_err(|error| format!("编码账号失败：{error}"))
        })
    }

    pub fn export(&self, id: Uuid, kind: &str, content: &str) -> Result<PathBuf, String> {
        let program = self.directory.parent().ok_or("无法定位程序目录")?;
        let path = program
            .join("exports")
            .join(format!("{id}-{kind}-{}.json", Uuid::new_v4()));
        write_atomic(&path, |writer| {
            writer
                .write_all(content.as_bytes())
                .map_err(|error| format!("写入导出文件失败：{error}"))
        })?;
        Ok(path)
    }

    pub fn remove(&self, id: Uuid) -> Result<(), String> {
        fs::remove_file(self.path(id)).map_err(|error| format!("删除本地账号失败：{error}"))
    }
}

fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let directory = path.parent().ok_or("无法定位写入目录")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("无法创建目录 {}：{error}", directory.display()))?;
    let temporary = directory.join(format!(".{}.tmp", Uuid::new_v4()));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("无法在 {} 创建暂存文件：{error}", directory.display()))?;
    let mut writer = BufWriter::new(file);
    let result = write(&mut writer).and_then(|_| {
        writer
            .flush()
            .and_then(|_| writer.get_ref().sync_all())
            .map_err(|error| format!("写入 {} 失败：{error}", path.display()))
    });
    drop(writer);
    let result = result.and_then(|_| {
        fs::rename(&temporary, path)
            .map_err(|error| format!("保存 {} 失败：{error}", path.display()))
    });
    if let Err(error) = result {
        if let Err(cleanup) = fs::remove_file(&temporary) {
            return Err(format!("{error}；清理暂存文件失败：{cleanup}"));
        }
        return Err(error);
    }
    Ok(())
}

fn read_record(path: &Path, id: Uuid) -> Result<StoredAccount, String> {
    let fail = |reason: &dyn std::fmt::Display| format!("无法读取账号 {id}：{reason}");
    let file = fs::File::open(path).map_err(|error| fail(&error))?;
    let record: StoredAccount =
        serde_json::from_reader(BufReader::new(file)).map_err(|error| fail(&error))?;
    if record.version != RECORD_VERSION
        || record.id != id
        || record.raw.schema_version != RAW_ACCOUNT_SCHEMA_VERSION
    {
        return Err(fail(&"账号文件版本或标识有误"));
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use account_core::{import::reliquary::import_reliquary_archive, reference::TraceCatalog};

    #[test]
    fn writes_complete_records_and_recovers_other_accounts_from_corruption() {
        let root = std::env::temp_dir().join(format!("hyacimicon-test-{}", Uuid::new_v4()));
        let directory = root.join("accounts");
        let files = AccountFiles::new(directory.clone());
        let original = include_str!("../tests/fixtures/synthetic-reliquary.json");
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/account/tests/fixtures/reference/traces.json");
        let catalog = TraceCatalog::from_path(&path).expect("synthetic catalog");
        let imported =
            import_reliquary_archive(original, "import", &catalog).expect("valid source");
        let id = Uuid::new_v4();
        let mut account = StoredAccount::new(
            id,
            "导入账号".into(),
            imported.account,
            Origin::Imported {
                filename: "fixture.json".into(),
                original_json: original.into(),
                report: Some(imported.report),
            },
        );
        files.save(&account).expect("write account");
        account.name = "更改名称".into();
        files.save(&account).expect("atomic replacement");
        let invalid = Uuid::new_v4();
        fs::write(files.path(invalid), "{").expect("corrupt sibling");
        let (records, warnings) = AccountFiles::new(directory.clone())
            .list()
            .expect("reload on restart");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains(&invalid.to_string()));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "更改名称");
        assert_eq!(records[0].raw, account.raw);
        let Origin::Imported {
            original_json,
            report,
            ..
        } = &records[0].origin
        else {
            panic!("import origin lost");
        };
        assert_eq!(original_json, original);
        assert!(!report
            .as_ref()
            .expect("Fribbels report")
            .trace_bindings_snapshot
            .is_empty());
        let export = files
            .export(id, "fribbels", original_json)
            .expect("portable export");
        assert_eq!(export.parent(), Some(root.join("exports").as_path()));
        assert_eq!(fs::read_to_string(&export).expect("read export"), original);
        fs::remove_file(export).expect("remove exported fixture");
        fs::remove_dir(root.join("exports")).expect("remove export directory");
        files.remove(id).expect("remove account");
        assert!(files.load(id).is_err());
        fs::remove_file(files.path(invalid)).expect("remove corrupt sibling");
        fs::remove_dir(directory).expect("remove test directory");
        fs::remove_dir(root).expect("remove test root");
    }
}
