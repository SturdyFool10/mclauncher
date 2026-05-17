use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use crate::{VTMPACK_EXTENSION, VtmpackManifest};

const XZ_MAGIC: &[u8] = &[0xfd, b'7', b'z', b'X', b'Z', 0x00];
const ZPAQ_MAGIC: &[u8] = &[
    0x37, 0x6b, 0x53, 0x74, 0xa0, 0x31, 0x83, 0xd3, 0x8c, 0xb2, 0x28, 0xb0, 0xd3,
];

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
    let mut archive = open_vtmpack_tar_archive(path)?;

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

pub fn open_vtmpack_tar_archive(path: &Path) -> Result<tar::Archive<Box<dyn Read>>, String> {
    let bytes =
        std::fs::read(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    if bytes.starts_with(XZ_MAGIC) {
        let decoder = xz2::read::XzDecoder::new(Cursor::new(bytes));
        return Ok(tar::Archive::new(Box::new(decoder)));
    }
    if bytes.starts_with(ZPAQ_MAGIC) {
        let mut tar_bytes = Vec::new();
        zpaq_rs::decompress_stream(Cursor::new(bytes), &mut tar_bytes).map_err(|err| {
            format!(
                "failed to decompress zpaq vtmpack {}: {err}",
                path.display()
            )
        })?;
        return Ok(tar::Archive::new(Box::new(Cursor::new(tar_bytes))));
    }

    Err(format!(
        "Unsupported Vertex pack compression in {}. Expected xz or zpaq.",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use crate::{VTMPACK_MANIFEST_VERSION, VtmpackInstanceMetadata};

    #[test]
    fn reads_zpaq_vtmpack_manifest() {
        let manifest = VtmpackManifest {
            format: "vtmpack".to_owned(),
            version: VTMPACK_MANIFEST_VERSION,
            instance: VtmpackInstanceMetadata {
                name: "ZPAQ Pack".to_owned(),
                game_version: "1.20.1".to_owned(),
                modloader: "Fabric".to_owned(),
                ..VtmpackInstanceMetadata::default()
            },
            ..VtmpackManifest::default()
        };
        let manifest_bytes = toml::to_string_pretty(&manifest)
            .expect("serialize test manifest")
            .into_bytes();
        let mut tar_bytes = Vec::new();
        {
            let mut archive = tar::Builder::new(&mut tar_bytes);
            let mut header = tar::Header::new_gnu();
            header.set_size(manifest_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, "manifest.toml", manifest_bytes.as_slice())
                .expect("append manifest");
            archive.finish().expect("finish tar");
        }

        let path = std::env::temp_dir().join(format!(
            "vertexlauncher-zpaq-vtmpack-test-{}.vtmpack",
            std::process::id()
        ));
        let mut file = fs::File::create(path.as_path()).expect("create zpaq test pack");
        zpaq_rs::compress_stream(
            Cursor::new(tar_bytes),
            &mut file,
            "1",
            Some("vtmpack.tar"),
            None,
        )
        .expect("compress zpaq test pack");
        file.flush().expect("flush zpaq test pack");

        let parsed = read_vtmpack_manifest(path.as_path()).expect("read zpaq manifest");
        let _ = fs::remove_file(path.as_path());

        assert_eq!(parsed.format, "vtmpack");
        assert_eq!(parsed.instance.name, "ZPAQ Pack");
    }
}
