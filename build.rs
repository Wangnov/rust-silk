use std::fs;
use std::path::{Path, PathBuf};

fn collect_c_sources(dir: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return sources,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("c") {
            sources.push(path);
        }
    }
    sources.sort();
    sources
}

fn main() {
    let vendor_root = PathBuf::from("vendor").join("silk");
    let src_dir = vendor_root.join("src");
    let include_dir = vendor_root.join("interface");

    println!("cargo:rerun-if-changed={}", src_dir.display());
    println!("cargo:rerun-if-changed={}", include_dir.display());

    let sources = collect_c_sources(&src_dir);
    if sources.is_empty() {
        panic!("no C sources found in {}", src_dir.display());
    }

    let mut build = cc::Build::new();
    build
        .files(sources)
        .include(&include_dir)
        .include(&src_dir)
        .flag_if_supported("-O3");

    build.compile("silk_sdk");
}
