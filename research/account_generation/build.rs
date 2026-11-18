use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let profiles = PathBuf::from("data/character");
    println!("cargo:rerun-if-changed={}", profiles.display());
    let mut paths = fs::read_dir(&profiles)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    let mut source = String::from("&[\n");
    for path in paths {
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid character profile filename",
                    )
                })?;
            let absolute = fs::canonicalize(&path)?;
            source.push_str(&format!("    ({id:?}, include_str!({absolute:?})),\n"));
        }
    }
    source.push_str("]\n");
    let output = env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("missing OUT_DIR"))?;
    fs::write(PathBuf::from(output).join("profiles.rs"), source)
}
