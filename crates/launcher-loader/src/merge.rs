use crate::models::LoaderProfile;
use launcher_types::LauncherErrorPayload;
use launcher_version::{MojangLibrary, MojangVersionDetail};

pub fn library_key(name: &str) -> String {
    let (clean, ext) = match name.find('@') {
        Some(idx) => (&name[..idx], name[idx + 1..].trim()),
        None => (name, "jar"),
    };

    let parts: Vec<&str> = clean.split(':').collect();
    let group = parts.first().copied().unwrap_or("");
    let artifact = parts.get(1).copied().unwrap_or("");
    let classifier = parts.get(3).copied().unwrap_or("");

    if ext.is_empty() || ext.eq_ignore_ascii_case("jar") {
        format!("{}:{}:{}", group, artifact, classifier)
    } else {
        format!("{}:{}:{}@{}", group, artifact, classifier, ext)
    }
}

pub fn merge_libraries(parent: &[MojangLibrary], child: &[MojangLibrary]) -> Vec<MojangLibrary> {
    let mut merged = Vec::with_capacity(parent.len() + child.len());

    for lib in parent {
        let p_key = library_key(&lib.name);
        let p_rules = lib
            .rules
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());
        let overridden = child.iter().any(|c| {
            if library_key(&c.name) != p_key {
                return false;
            }
            match (&c.rules, &p_rules) {
                (None, _) => true,
                (Some(cr), Some(pr)) => serde_json::to_string(cr).ok().as_ref() == Some(pr),
                (Some(_), None) => false,
            }
        });
        if !overridden {
            merged.push(lib.clone());
        }
    }

    merged.extend(child.iter().cloned());
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

    if let Some(ref jv) = profile.java_version {
        merged.java_version = Some(jv.clone());
    }

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
    use launcher_version::{MojangRule, MojangRuleOs};

    #[test]
    fn test_h13_merge_libraries_preserves_child_per_os_and_removes_parent_duplicates() {
        let win_rule = vec![MojangRule {
            action: "allow".to_string(),
            os: Some(MojangRuleOs {
                name: Some("windows".to_string()),
                arch: None,
                version: None,
            }),
            features: None,
        }];
        let linux_rule = vec![MojangRule {
            action: "allow".to_string(),
            os: Some(MojangRuleOs {
                name: Some("linux".to_string()),
                arch: None,
                version: None,
            }),
            features: None,
        }];

        let parent = vec![
            MojangLibrary {
                name: "org.ow2.asm:asm:9.3".to_string(),
                ..Default::default()
            },
            MojangLibrary {
                name: "org.ow2.asm:asm:9.3".to_string(),
                ..Default::default()
            },
        ];
        let child = vec![
            MojangLibrary {
                name: "org.ow2.asm:asm:9.6".to_string(),
                ..Default::default()
            },
            MojangLibrary {
                name: "org.lwjgl:lwjgl:3.3.3:natives-windows".to_string(),
                rules: Some(win_rule),
                ..Default::default()
            },
            MojangLibrary {
                name: "org.lwjgl:lwjgl:3.3.3:natives-windows".to_string(),
                rules: Some(linux_rule),
                ..Default::default()
            },
        ];

        let merged = merge_libraries(&parent, &child);
        let asm_entries: Vec<_> = merged
            .iter()
            .filter(|l| l.name.starts_with("org.ow2.asm:asm:"))
            .collect();
        assert_eq!(asm_entries.len(), 1);
        assert_eq!(asm_entries[0].name, "org.ow2.asm:asm:9.6");

        let lwjgl_entries: Vec<_> = merged
            .iter()
            .filter(|l| l.name == "org.lwjgl:lwjgl:3.3.3:natives-windows")
            .collect();
        assert_eq!(lwjgl_entries.len(), 2);
    }

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
