//! Execute the shipped CLI against the public synthetic example.
use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn cli_executes_loss_then_win_and_preserves_full_width_seed_replay() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/two-battles.json");
    let execute = || {
        let output = Command::new(env!("CARGO_BIN_EXE_hcwa-run"))
            .arg("simulate-run")
            .arg(&spec)
            .arg(u64::MAX.to_string())
            .output()
            .expect("start actual run CLI");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        output.stdout
    };
    let first = execute();
    assert_eq!(
        first,
        execute(),
        "same request must reproduce the complete JSON result"
    );
    let report: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(report["master_seed"], u64::MAX.to_string());
    assert!(!report["seed_derivation"]
        .as_str()
        .unwrap()
        .trim()
        .is_empty());
    assert_eq!(report["observation"]["status"], "completed");
    assert_eq!(report["observation"]["resources"]["hp"], 23.0);
    assert_eq!(report["observation"]["resources"]["gold"], 17);
    assert_eq!(
        report["observation"]["account_config_id"],
        "synthetic-account-v1"
    );
    assert_eq!(report["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(report["settlements"].as_array().unwrap().len(), 2);
    assert_eq!(report["transitions"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["attempts"][0]["resolution"]["result"]["outcome"],
        "defeat"
    );
    assert_eq!(
        report["attempts"][1]["resolution"]["result"]["outcome"],
        "victory"
    );
    for attempt in report["attempts"].as_array().unwrap() {
        attempt["resolution"]["result"]["seed"]
            .as_str()
            .expect("seed is a decimal string")
            .parse::<u64>()
            .unwrap();
    }
    assert_eq!(report["transitions"][0]["reward"], Value::Null);
    assert_eq!(report["transitions"][0]["terminated"], false);
    assert_eq!(report["transitions"][1]["terminated"], true);
}
