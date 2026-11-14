use crate::models::LoaderProfile;
use launcher_types::LauncherErrorPayload;
use launcher_version::{MojangLibrary, MojangVersionDetail};

pub fn library_key(name: &str) -> String {
    let clean = if let Some(idx) = name.find('@') {
        &name[..idx]
    } else {
        name
    };

    let parts: Vec<&str> = clean.split(':').collect();
    let group = parts.first().copied().unwrap_or("");
    let artifact = parts.get(1).copied().unwrap_or("");
    let classifier = parts.get(3).copied().unwrap_or("");

    format!("{}:{}:{}", group, artifact, classifier)
}

pub fn merge_libraries(parent: &[MojangLibrary], child: &[MojangLibrary]) -> Vec<MojangLibrary> {
    let mut merged = parent.to_vec();

    for lib in child {
        let key = library_key(&lib.name);
        if let Some(pos) = merged.iter().position(|c| library_key(&c.name) == key) {
            merged.remove(pos);
        }
        merged.push(lib.clone());
    }

    merged
}

pub fn merge_version_detail(
    parent: &MojangVersionDetail,
    profile: &LoaderProfile,
) -> Result<MojangVersionDetail, LauncherErrorPayload> {
    if profile.inherits_from != parent.id {
        return Err(LauncherErrorPayload::invalid_input(format!(
            "Profil loader {} mewarisi {}, bukan {}.",
            profile.id, profile.inherits_from, parent.id
        )));
    }

    let libraries = merge_libraries(&parent.libraries, &profile.libraries);

    let mut merged = parent.clone();
    merged.id = profile.id.clone();
    merged.inherits_from = Some(profile.inherits_from.clone());
    merged.libraries = libraries;

    if let Some(ref mc) = profile.main_class {
        merged.main_class = Some(mc.clone());
    }

    // Merge arguments jika ada
    if parent.arguments.is_some() || profile.arguments.is_some() {
        let mut game_args = Vec::new();
        let mut jvm_args = Vec::new();

        if let Some(ref p_args) = parent.arguments {
            if let Some(g) = p_args.get("game").and_then(|v| v.as_array()) {
                game_args.extend(g.clone());
            }
            if let Some(j) = p_args.get("jvm").and_then(|v| v.as_array()) {
                jvm_args.extend(j.clone());
            }
        }

        if let Some(ref c_args) = profile.arguments {
            if let Some(g) = c_args.get("game").and_then(|v| v.as_array()) {
                game_args.extend(g.clone());
            }
            if let Some(j) = c_args.get("jvm").and_then(|v| v.as_array()) {
                jvm_args.extend(j.clone());
            }
        }

        merged.arguments = Some(serde_json::json!({
            "game": game_args,
            "jvm": jvm_args,
        }));
    }

    if let Some(ref legacy) = profile.minecraft_arguments {
        merged.minecraft_arguments = Some(legacy.clone());
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_key() {
        assert_eq!(
            library_key("net.fabricmc:sponge-mixin:0.12.5+mixin.0.8.5"),
            "net.fabricmc:sponge-mixin:"
        );
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.1:natives-windows"),
            "org.lwjgl:lwjgl:natives-windows"
        );
        assert_eq!(
            library_key("com.mojang:patchy:1.1@jar"),
            "com.mojang:patchy:"
        );
    }
}
