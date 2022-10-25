use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap();
    let out_dir: PathBuf = env::var_os("OUT_DIR").expect("OUT_DIR not set").into();
    let out_file = &out_dir.join("target");
    fs::write(out_file, format!("\"{target}\""))
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_file.display()));
}
