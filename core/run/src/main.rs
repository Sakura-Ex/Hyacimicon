use run_core::{RunEnv, RunSpec, RunStatus, SampleRequest};
use std::{
    env, fs,
    io::{self, Read, Write},
};

fn execute() -> Result<run_core::RunReport, String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 || args[1] != "simulate-run" {
        return Err("usage: hcwa-run simulate-run RUN_SPEC.json SEED".into());
    }
    let seed = args[3].parse::<u64>().map_err(|e| format!("seed: {e}"))?;
    let mut bytes = Vec::new();
    fs::File::open(&args[2])
        .map_err(|e| format!("run spec: {e}"))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("run spec exceeds 16 MiB input limit".into());
    }
    let spec: RunSpec = serde_json::from_slice(&bytes).map_err(|e| format!("run schema: {e}"))?;
    let mut run = RunEnv::new(spec, "cli-run".into(), seed)?;
    while run.observe().status == RunStatus::Ready {
        let obs = run.observe();
        run.step(
            &SampleRequest {
                request_id: format!("cli-node-{}", obs.node_index),
                node_id: obs.current_node_id.ok_or("ready run lacks current node")?,
                expected_revision: obs.revision,
                trace: true,
            },
            &mut || false,
        )?;
    }
    Ok(run.report())
}

fn main() {
    let result = execute().and_then(|report| {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &report).map_err(|e| e.to_string())?;
        writeln!(stdout).map_err(|e| e.to_string())
    });
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
