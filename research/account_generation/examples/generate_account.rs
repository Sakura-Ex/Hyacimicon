//! 从外部资料生成账号，并将 RawAccount JSON 写到标准输出。

use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use account_core::reference::TraceCatalog;
use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, ReferenceOverrides, SeedableRng, generate_account,
};

/// 前两个位置参数为资料目录与行迹 JSON，其后为请求和随机种子。
/// 将原始账号 JSON 写到标准输出，将种子、金数、遗器均分和搜索次数写到标准错误。
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let usage = "用法: generate_account <参考数据目录> <行迹 JSON> [请求.json] [随机种子]";
    let reference_dir = PathBuf::from(args.next().ok_or(usage)?);
    let trace_path = PathBuf::from(args.next().ok_or(usage)?);
    let mut seed = 42;
    let mut request = AccountRequest {
        gold_count: Some(20.0),
        core_character_ids: vec!["1310".to_owned(), "1225".to_owned()],
        resource_concentration: 0.7,
        min_mean_effective_rolls: Some(4.5),
        max_mean_effective_rolls: Some(5.0),
        ..AccountRequest::with_random_id(12)
    };
    if let Some(path) = args.next() {
        let source = fs::read_to_string(path)?;
        request = serde_json::from_str(source.trim_start_matches('\u{feff}'))?;
    }
    if let Some(value) = args.next() {
        seed = value.parse()?;
    }
    if args.next().is_some() {
        return Err(usage.into());
    }
    let traces = TraceCatalog::from_path(&trace_path)?;
    let reference = AccountReference::from_dir(
        &reference_dir,
        Some(&traces),
        &ReferenceOverrides::default(),
    )?;
    let result = generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(seed))?;
    eprintln!(
        "seed={seed} gold={} mean_effective_rolls={:.4} attempts={}",
        result.gold_count(),
        result.mean_effective_rolls(),
        result.attempts,
    );
    let mut output = io::stdout().lock();
    serde_json::to_writer_pretty(&mut output, &result.account)?;
    output.flush()?;
    Ok(())
}
