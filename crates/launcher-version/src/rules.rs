use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangRuleOs {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangRule {
    pub action: String, // "allow" atau "disallow"
    pub os: Option<MojangRuleOs>,
    pub features: Option<HashMap<String, bool>>,
}

pub fn host_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "osx"
    }
}

pub fn host_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "arm64"
    }
}

pub fn host_arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

pub fn is_rule_matching_os(rule: &MojangRule, current_os: &str, current_arch: &str) -> bool {
    if let Some(ref os) = rule.os {
        if let Some(ref name) = os.name {
            if !name.eq_ignore_ascii_case(current_os) {
                return false;
            }
        }

        if let Some(ref arch) = os.arch {
            if !arch.eq_ignore_ascii_case(current_arch) {
                return false;
            }
        }
    }

    // Mengabaikan fitur game khusus (demo_user, has_custom_resolution, dll) kecuali dievaluasi eksplisit
    if let Some(ref features) = rule.features {
        if !features.is_empty() {
            return false;
        }
    }

    true
}

pub fn is_library_allowed(
    rules: Option<&[MojangRule]>,
    current_os: &str,
    current_arch: &str,
) -> bool {
    let rules = match rules {
        Some(r) if !r.is_empty() => r,
        _ => return true, // Jika tidak ada aturan, default diizinkan
    };

    let mut allowed = false;

    for rule in rules {
        if is_rule_matching_os(rule, current_os, current_arch) {
            if rule.action == "allow" {
                allowed = true;
            } else if rule.action == "disallow" {
                allowed = false;
            }
        }
    }

    allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_evaluation() {
        let rules_windows = vec![MojangRule {
            action: "allow".to_string(),
            os: Some(MojangRuleOs {
                name: Some("windows".to_string()),
                arch: None,
                version: None,
            }),
            features: None,
        }];
        assert!(is_library_allowed(Some(&rules_windows), "windows", "x64"));
        assert!(!is_library_allowed(Some(&rules_windows), "osx", "x64"));

        let rules_disallow_osx = vec![
            MojangRule {
                action: "allow".to_string(),
                os: None,
                features: None,
            },
            MojangRule {
                action: "disallow".to_string(),
                os: Some(MojangRuleOs {
                    name: Some("osx".to_string()),
                    arch: None,
                    version: None,
                }),
                features: None,
            },
        ];
        assert!(is_library_allowed(
            Some(&rules_disallow_osx),
            "windows",
            "x64"
        ));
        assert!(!is_library_allowed(Some(&rules_disallow_osx), "osx", "x64"));
    }
}
