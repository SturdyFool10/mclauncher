use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{VTMPACK_EXTENSION, VtmpackManifest};

#[must_use]
pub fn default_vtmpack_file_name(instance_name: &str) -> String {
    let mut out = String::new();
    for ch in instance_name.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '.' {
            out.push('-');
        }
    }
    let base = out.trim_matches('-');
    if base.is_empty() {
        format!("instance.{VTMPACK_EXTENSION}")
    } else {
        format!("{base}.{VTMPACK_EXTENSION}")
    }
}

#[must_use]
pub fn enforce_vtmpack_extension(mut path: PathBuf) -> PathBuf {
    let has_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(VTMPACK_EXTENSION));
    if !has_extension {
        path.set_extension(VTMPACK_EXTENSION);
    }
    path
}

pub fn read_vtmpack_manifest(path: &Path) -> Result<VtmpackManifest, String> {
    let file = std::fs::File::open(path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?
    {
        let mut entry = entry.map_err(|err| format!("failed to read archive entry: {err}"))?;
        let entry_path = entry
            .path()
            .map_err(|err| format!("failed to decode archive path: {err}"))?;
        if entry_path == Path::new("manifest.toml") {
            let mut raw = String::new();
            entry
                .read_to_string(&mut raw)
                .map_err(|err| format!("failed to read manifest.toml: {err}"))?;
            return toml::from_str(&raw)
                .map_err(|err| format!("failed to parse vtmpack manifest: {err}"));
        }
    }

    Err(format!(
        "No manifest.toml found in Vertex pack {}",
        path.display()
    ))
}
