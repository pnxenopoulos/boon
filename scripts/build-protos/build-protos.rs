//! Run: cargo run --manifest-path scripts/build-protos/Cargo.toml --bin build-boon-protos
//! Maintainers-only: generates prebuilt Rust into crates/boon-proto/src/generated/proto.rs

use std::{fs, io::Write, path::{Path, PathBuf}};

fn main() {
    // Config
    let manifest   = Path::new("crates/boon-proto/proto/manifest.txt");
    let proto_root = Path::new("crates/boon-proto/proto");
    let dest_dir   = Path::new("crates/boon-proto/src/generated");
    let out_file   = dest_dir.join("proto.rs"); // <-- single output file

    if !manifest.exists() {
        eprintln!("manifest not found at {}", manifest.display());
        std::process::exit(1);
    }

    // Collect protos from manifest
    let content = fs::read_to_string(manifest).expect("read manifest");
    let mut protos: Vec<PathBuf> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') { continue; }
        let p = proto_root.join(entry);
        if !p.exists() {
            eprintln!("missing proto: {} (line {})", p.display(), i + 1);
            std::process::exit(1);
        }
        protos.push(p);
    }
    if protos.is_empty() {
        eprintln!("manifest is empty");
        std::process::exit(1);
    }

    // Generate into a temp dir
    let tmp = tempfile::tempdir().expect("tmp dir");
    let out_tmp = tmp.path().to_path_buf();

    let mut cfg = prost_build::Config::new();
    cfg.out_dir(&out_tmp);
    cfg.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");

    cfg.compile_protos(&protos, &[proto_root]).expect("prost compile");

    // Write src/generated/proto.rs with all generated code
    fs::create_dir_all(dest_dir).expect("create dest");

    // Remove any old .rs in generated/
    for entry in fs::read_dir(dest_dir).unwrap_or_else(|_| panic!("read {}", dest_dir.display())) {
        let p = entry.expect("dirent").path();
        if p == out_file { continue; }
        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            fs::remove_file(&p).expect("remove old generated");
        }
    }

    // Collect prost outputs (e.g., "*.rs), sort
    let mut files: Vec<PathBuf> = fs::read_dir(&out_tmp)
        .expect("read tmp out")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension().and_then(|s| s.to_str()) == Some("rs")).then_some(p)
        })
        .collect();
    files.sort(); // stable concatenation order

    // Concatenate all generated files into proto.rs
    let mut out = fs::File::create(&out_file).expect("create proto.rs");
    writeln!(
        out,
        "// @generated — DO NOT EDIT. See scripts/build-protos/build-protos.rs\n"
    ).unwrap();

    for fpath in files {
        let contents = fs::read_to_string(&fpath)
            .unwrap_or_else(|e| panic!("read {}: {}", fpath.display(), e));
        out.write_all(contents.as_bytes()).unwrap();
    }

    println!("Wrote {}", out_file.display());
}
