use launcher_version::MojangLibrary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderGameSupport {
    pub fabric: Option<Vec<String>>,
    pub quilt: Option<Vec<String>>,
    pub forge: Option<Vec<String>>,
    pub neoforge: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoaderVersionInfo {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderProfile {
    pub id: String,
    #[serde(default)]
    pub inherits_from: String,
    pub main_class: Option<String>,
    #[serde(rename = "type")]
    pub profile_type: Option<String>,
    pub release_time: Option<String>,
    #[serde(default)]
    pub libraries: Vec<MojangLibrary>,
    pub arguments: Option<serde_json::Value>,
    pub minecraft_arguments: Option<String>,
}

pub fn compare_loader_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let l_clean = left.split('+').next().unwrap_or(left);
    let r_clean = right.split('+').next().unwrap_or(right);

    let (l_core, l_pre) = match l_clean.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (l_clean, None),
    };
    let (r_core, r_pre) = match r_clean.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (r_clean, None),
    };

    let l_parts: Vec<&str> = l_core.split('.').collect();
    let r_parts: Vec<&str> = r_core.split('.').collect();
    let min_len = l_parts.len().min(r_parts.len());

    for i in 0..min_len {
        let l_num = l_parts.get(i).and_then(|s| s.parse::<u64>().ok());
        let r_num = r_parts.get(i).and_then(|s| s.parse::<u64>().ok());

        match (l_num, r_num) {
            (Some(l), Some(r)) => {
                if l != r {
                    return r.cmp(&l); // descending: terbaru lebih dulu
                }
            }
            _ => {
                let l_str = l_parts.get(i).copied().unwrap_or("");
                let r_str = r_parts.get(i).copied().unwrap_or("");
                if l_str != r_str {
                    return r_str.cmp(l_str);
                }
            }
        }
    }

    if l_parts.len() != r_parts.len() {
        return r_parts.len().cmp(&l_parts.len());
    }

    match (l_pre, r_pre) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less, // release is newer than prerelease
        (Some(_), None) => std::cmp::Ordering::Greater, // prerelease is older than release
        (Some(lp), Some(rp)) => {
            let lp_parts: Vec<&str> = lp.split('.').collect();
            let rp_parts: Vec<&str> = rp.split('.').collect();
            let p_len = lp_parts.len().min(rp_parts.len());

            for i in 0..p_len {
                let l_num = lp_parts.get(i).and_then(|s| s.parse::<u64>().ok());
                let r_num = rp_parts.get(i).and_then(|s| s.parse::<u64>().ok());

                match (l_num, r_num) {
                    (Some(l), Some(r)) => {
                        if l != r {
                            return r.cmp(&l);
                        }
                    }
                    (Some(_), None) => return std::cmp::Ordering::Greater,
                    (None, Some(_)) => return std::cmp::Ordering::Less,
                    (None, None) => {
                        let l_s = lp_parts.get(i).copied().unwrap_or("");
                        let r_s = rp_parts.get(i).copied().unwrap_or("");
                        if l_s != r_s {
                            return r_s.cmp(l_s);
                        }
                    }
                }
            }
            rp_parts.len().cmp(&lp_parts.len())
        }
    }
}
