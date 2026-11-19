//! 从内置资料生成账号，并将 RawAccount JSON 写到标准输出。

use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
};

use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, SeedableRng, generate_account,
};

/// 第一个位置参数为请求 JSON 路径，第二个为随机种子（默认 42）。
/// 将原始账号 JSON 写到标准输出，将种子、金数、遗器均分和搜索次数写到标准错误。
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut seed = 42;
    let mut request = AccountRequest {
        gold_count: Some(20.0),
        core_character_ids: vec!["1310".to_owned(), "1225".to_owned()],
        resource_concentration: 0.7,
        min_mean_effective_rolls: Some(4.5),
        max_mean_effective_rolls: Some(5.0),
        ..AccountRequest::new("account-001", 12)
    };
    if let Some(path) = args.next() {
        let source = fs::read_to_string(path)?;
        request = serde_json::from_str(source.trim_start_matches('\u{feff}'))?;
    }
    if let Some(value) = args.next() {
        seed = value.parse()?;
    }
    if args.next().is_some() {
        return Err("用法: generate_account [请求.json] [随机种子]".into());
    }
    let reference = AccountReference::bundled()?;
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
