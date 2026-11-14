use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use launcher_types::LauncherErrorPayload;

pub const OFFICIAL_STEVE_PNG_BASE64: &str =
    "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\
8LsB1DHaeH/5actqe2IA5IAAAAASUVORK5CYII=";

pub const OFFICIAL_ALEX_PNG_BASE64: &str =
    "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";

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean = input.trim_matches(|c: char| c.is_whitespace() || c == '=');
    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in clean.as_bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

pub fn get_default_skin_bytes(is_slim: bool) -> Vec<u8> {
    let b64 = if is_slim {
        OFFICIAL_ALEX_PNG_BASE64
    } else {
        OFFICIAL_STEVE_PNG_BASE64
    };
    decode_base64(b64).unwrap_or_default()
}

pub fn build_skin_resourcepack(
    skin_png_bytes: &[u8],
    out_zip_path: &Path,
) -> Result<(), LauncherErrorPayload> {
    let parent = out_zip_path.parent().ok_or_else(|| {
        LauncherErrorPayload::invalid_input("Path output resource pack tidak memiliki induk.")
    })?;
    launcher_security::ensure_secure_dir(parent, false)?;

    if let Ok(meta) = std::fs::symlink_metadata(out_zip_path) {
        if meta.file_type().is_symlink() {
            return Err(LauncherErrorPayload::invalid_input(
                "Target resource pack tidak boleh berupa symlink.",
            ));
        }
    }

    let tmp_name = format!(
        ".pack.tmp-{}.zip",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let tmp_path = parent.join(tmp_name);
    let file = launcher_security::safe_create_new_file(&tmp_path, false)?;

    let write_res: Result<(), LauncherErrorPayload> = (|| {
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let mcmeta = r#"{
  "pack": {
    "pack_format": 15,
    "supported_formats": {"min_inclusive": 1, "max_inclusive": 99},
    "description": "Launcher Skin Resource Pack"
  }
}"#;
        zip.start_file("pack.mcmeta", options).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menulis pack.mcmeta: {}", e))
        })?;
        zip.write_all(mcmeta.as_bytes()).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menulis data pack.mcmeta: {}", e))
        })?;

        let paths = [
            "assets/minecraft/textures/entity/steve.png",
            "assets/minecraft/textures/entity/alex.png",
            "assets/minecraft/textures/entity/player/wide/steve.png",
            "assets/minecraft/textures/entity/player/wide/alex.png",
            "assets/minecraft/textures/entity/player/wide/ari.png",
            "assets/minecraft/textures/entity/player/wide/efe.png",
            "assets/minecraft/textures/entity/player/wide/kai.png",
            "assets/minecraft/textures/entity/player/wide/makena.png",
            "assets/minecraft/textures/entity/player/wide/noor.png",
            "assets/minecraft/textures/entity/player/wide/sunny.png",
            "assets/minecraft/textures/entity/player/wide/zuri.png",
            "assets/minecraft/textures/entity/player/slim/steve.png",
            "assets/minecraft/textures/entity/player/slim/alex.png",
            "assets/minecraft/textures/entity/player/slim/ari.png",
            "assets/minecraft/textures/entity/player/slim/efe.png",
            "assets/minecraft/textures/entity/player/slim/kai.png",
            "assets/minecraft/textures/entity/player/slim/makena.png",
            "assets/minecraft/textures/entity/player/slim/noor.png",
            "assets/minecraft/textures/entity/player/slim/sunny.png",
            "assets/minecraft/textures/entity/player/slim/zuri.png",
        ];

        for p in paths {
            zip.start_file(p, options).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal menulis entri skin {}: {}", p, e))
            })?;
            zip.write_all(skin_png_bytes).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal menulis isi skin {}: {}", p, e))
            })?;
        }

        zip.finish().map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menyelesaikan pembuatan ZIP: {}", e))
        })?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    std::fs::rename(&tmp_path, out_zip_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        LauncherErrorPayload::file_system(format!("Gagal menyimpan berkas resource pack: {}", e))
    })?;

    Ok(())
}

pub fn activate_resourcepack_in_options(
    options_path: &Path,
    pack_file_name: &str,
) -> std::io::Result<()> {
    let pack_entry = format!("\"file/{}\"", pack_file_name);
    let contents = if options_path.exists() {
        std::fs::read_to_string(options_path)?
    } else {
        String::new()
    };

    let mut lines: Vec<String> = contents.lines().map(|s| s.to_string()).collect();
    let mut found = false;

    for line in &mut lines {
        if line.starts_with("resourcePacks:") {
            found = true;
            if !line.contains(&pack_entry) {
                if let Some(open_idx) = line.find('[') {
                    if let Some(close_idx) = line.rfind(']') {
                        let inside = line[open_idx + 1..close_idx].trim();
                        if inside.is_empty() {
                            *line = format!("resourcePacks:[{}]", pack_entry);
                        } else {
                            let after = open_idx + 1;
                            line.insert_str(after, &format!("{},", pack_entry));
                        }
                    } else {
                        *line = format!("resourcePacks:[{}]", pack_entry);
                    }
                } else {
                    *line = format!("resourcePacks:[{}]", pack_entry);
                }
            }
            break;
        }
    }

    if !found {
        lines.push(format!("resourcePacks:[{},\"vanilla\"]", pack_entry));
    }

    std::fs::write(options_path, lines.join("\n") + "\n")?;
    Ok(())
}
