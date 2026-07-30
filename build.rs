use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));
    let asset_dir = manifest_dir.join("web/dist/assets");
    println!("cargo:rerun-if-changed={}", asset_dir.display());

    let mut assets = Vec::new();
    collect_assets(&asset_dir, &asset_dir, &mut assets);
    assets.sort_by(|left, right| left.0.cmp(&right.0));

    let mut generated = String::from(
        "#[cfg(test)]\n\
         pub(super) const EMBEDDED_ASSET_PATHS: &[&str] = &[\n",
    );
    for (name, _) in &assets {
        generated.push_str(&format!("    {name:?},\n"));
    }
    generated.push_str(
        "];\n\n\
         pub(super) fn embedded_asset(path: &str) -> Option<(&'static [u8], &'static str)> {\n\
         \x20   match path {\n",
    );
    for (name, path) in &assets {
        println!("cargo:rerun-if-changed={}", path.display());
        generated.push_str(&format!(
            "        {:?} => Some((include_bytes!({:?}), {:?})),\n",
            name,
            path,
            content_type(path)
        ));
    }
    generated.push_str("        _ => None,\n    }\n}\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"))
        .join("embedded_assets.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", output.display()));
}

fn collect_assets(directory: &Path, root: &Path, assets: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("failed to read frontend asset entry").path();
        if path.is_dir() {
            collect_assets(&path, root, assets);
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("frontend asset is outside its root");
            let name = relative
                .iter()
                .map(|component| {
                    component.to_str().unwrap_or_else(|| {
                        panic!("frontend asset name is not valid UTF-8: {}", path.display())
                    })
                })
                .collect::<Vec<_>>()
                .join("/");
            assets.push((name, path));
        } else {
            panic!("frontend asset is not a regular file: {}", path.display());
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => panic!(
            "frontend asset has no configured content type: {}",
            path.display()
        ),
    }
}
