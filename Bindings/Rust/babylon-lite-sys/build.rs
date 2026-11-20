//! Builds LiteLayer and LiteShaderCompiler with CMake and links their complete
//! static closure. The link line comes from the CMake file API for the
//! `LiteLinkProbe` executable, so it stays correct as native dependencies change.
//!
//! Environment:
//! - `BABYLON_NATIVE_SOURCE_DIR`: BabylonNative checkout (default: this repository).
//! - `BABYLON_LITE_CMAKE_BUILD_DIR`: persistent CMake build directory, so the
//!   native dependencies survive `cargo clean` (default: `$OUT_DIR/native`).
//! - `CMAKE_GENERATOR`: honored by CMake on first configure.

use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PROBE_TARGET: &str = "LiteLinkProbe";
const CONFIG: &str = "Release";

fn main() {
    let target = env::var("TARGET").unwrap();
    if target != "x86_64-pc-windows-msvc" {
        panic!(
            "babylon-lite-sys currently supports only x86_64-pc-windows-msvc: \
             the runtime shader compiler implements the D3D11 backend only (target: {target})"
        );
    }
    if env::var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|f| f.split(',').any(|f| f == "crt-static")) {
        panic!(
            "babylon-lite-sys links native libraries built with the dynamic CRT (/MD); disable crt-static"
        );
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let source_dir = env::var_os("BABYLON_NATIVE_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("..").join("..").join(".."));
    let source_dir = absolute(&source_dir);
    let build_dir = env::var_os("BABYLON_LITE_CMAKE_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join("native"));
    let build_dir = absolute(&build_dir);

    for variable in [
        "BABYLON_NATIVE_SOURCE_DIR",
        "BABYLON_LITE_CMAKE_BUILD_DIR",
        "CMAKE_GENERATOR",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    for path in [
        manifest_dir.join("native"),
        source_dir.join("Core").join("LiteLayer"),
        source_dir.join("Core").join("LiteShaderCompiler"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let query = build_dir
        .join(".cmake")
        .join("api")
        .join("v1")
        .join("query");
    fs::create_dir_all(&query).unwrap();
    fs::write(query.join("codemodel-v2"), "").unwrap();

    if !build_dir.join("CMakeCache.txt").exists() || find_reply_index(&build_dir).is_none() {
        run(Command::new("cmake")
            .arg("-S")
            .arg(manifest_dir.join("native"))
            .arg("-B")
            .arg(&build_dir)
            .arg(format!(
                "-DBABYLON_NATIVE_SOURCE_DIR={}",
                source_dir.display()
            ))
            .arg(format!("-DCMAKE_BUILD_TYPE={CONFIG}")));
    }
    run(Command::new("cmake").arg("--build").arg(&build_dir).args([
        "--config",
        CONFIG,
        "--target",
        PROBE_TARGET,
        "--parallel",
    ]));

    let libraries = probe_link_libraries(&build_dir);
    emit_link_directives(&build_dir, &out_dir, &libraries);
}

fn absolute(path: &Path) -> PathBuf {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    // CMake does not accept verbatim (\\?\) paths.
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(stripped) => PathBuf::from(stripped),
        None => path,
    }
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to start {command:?}: {error} (is CMake on PATH?)"));
    if !status.success() {
        panic!("{command:?} failed with {status}");
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

fn find_reply_index(build_dir: &Path) -> Option<PathBuf> {
    let reply = build_dir
        .join(".cmake")
        .join("api")
        .join("v1")
        .join("reply");
    fs::read_dir(reply)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("index-") && n.ends_with(".json"))
        })
        .max()
}

/// Returns the probe's link-line library fragments in CMake's order.
fn probe_link_libraries(build_dir: &Path) -> Vec<String> {
    let index_path = find_reply_index(build_dir).expect("CMake file API produced no reply");
    let reply_dir = index_path.parent().unwrap();
    let index = read_json(&index_path);
    let codemodel_file = index["objects"]
        .as_array()
        .and_then(|objects| objects.iter().find(|o| o["kind"] == "codemodel"))
        .and_then(|o| o["jsonFile"].as_str())
        .expect("CMake file API reply has no codemodel");
    let codemodel = read_json(&reply_dir.join(codemodel_file));
    let configurations = codemodel["configurations"].as_array().unwrap();
    let configuration = configurations
        .iter()
        .find(|c| c["name"] == CONFIG)
        .or_else(|| (configurations.len() == 1).then(|| &configurations[0]))
        .unwrap_or_else(|| panic!("CMake codemodel has no {CONFIG} configuration"));
    let target_file = configuration["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == PROBE_TARGET)
        .and_then(|t| t["jsonFile"].as_str())
        .unwrap_or_else(|| panic!("CMake codemodel has no {PROBE_TARGET} target"));
    let target = read_json(&reply_dir.join(target_file));
    target["link"]["commandFragments"]
        .as_array()
        .expect("probe target has no link command fragments")
        .iter()
        .filter(|f| f["role"] == "libraries" || f["role"] == "libraryPath")
        .filter_map(|f| f["fragment"].as_str().map(str::to_owned))
        .collect()
}

fn emit_link_directives(build_dir: &Path, out_dir: &Path, fragments: &[String]) {
    let alias_dir = out_dir.join("link-aliases");
    let mut search_dirs = BTreeSet::new();
    let mut seen_stems: HashMap<String, usize> = HashMap::new();
    let mut linked_any = false;

    for fragment in fragments {
        for item in split_fragment(fragment) {
            if let Some(dir) = item
                .strip_prefix("/LIBPATH:")
                .or_else(|| item.strip_prefix("-L"))
            {
                println!("cargo:rustc-link-search=native={dir}");
                continue;
            }
            if item.starts_with('/') || item.starts_with('-') {
                continue;
            }
            let path = Path::new(&item);
            let is_file_path = path.components().count() > 1;
            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            let stem = file_name
                .strip_suffix(".lib")
                .unwrap_or(&file_name)
                .to_owned();
            if !is_file_path {
                println!("cargo:rustc-link-lib=dylib={stem}");
                continue;
            }

            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                build_dir.join(path)
            };
            assert!(
                path.exists(),
                "linked library does not exist: {}",
                path.display()
            );
            let count = seen_stems.entry(stem.to_ascii_lowercase()).or_default();
            *count += 1;
            let (dir, name) = if *count == 1 {
                (path.parent().unwrap().to_path_buf(), stem)
            } else {
                // Same-named archives in different directories would resolve to
                // whichever search path comes first; give duplicates unique names.
                fs::create_dir_all(&alias_dir).unwrap();
                let alias = format!("{stem}__{count}");
                fs::copy(&path, alias_dir.join(format!("{alias}.lib"))).unwrap();
                (alias_dir.clone(), alias)
            };
            if search_dirs.insert(dir.clone()) {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
            println!("cargo:rustc-link-lib=static:-bundle={name}");
            linked_any = true;
        }
    }
    assert!(
        linked_any,
        "the probe link line contained no static libraries"
    );
}

/// Splits a native-shell command fragment into arguments, honoring quotes.
fn split_fragment(fragment: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in fragment.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    items.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        items.push(current);
    }
    items
}
