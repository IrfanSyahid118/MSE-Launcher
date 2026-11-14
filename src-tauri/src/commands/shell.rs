use launcher_types::LauncherErrorPayload;
use tauri_plugin_opener::OpenerExt;
use url::Url;

const EXTERNAL_HOST_ALLOWLIST: &[&str] = &[
    "modrinth.com",
    "curseforge.com",
    "microsoft.com",
    "microsoftonline.com",
    "live.com",
    "minecraft.net",
];

pub fn validate_external_url(url: &str) -> Result<Url, LauncherErrorPayload> {
    let parsed =
        Url::parse(url).map_err(|_| LauncherErrorPayload::invalid_input("URL tidak valid."))?;

    if parsed.scheme() != "https" {
        return Err(LauncherErrorPayload::invalid_input(
            "Hanya protokol HTTPS yang diizinkan.",
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| LauncherErrorPayload::invalid_input("URL tidak memiliki host."))?
        .to_lowercase();

    let is_allowed = EXTERNAL_HOST_ALLOWLIST
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed)));

    if !is_allowed {
        return Err(LauncherErrorPayload::invalid_input(format!(
            "Domain '{}' tidak diizinkan untuk dibuka di browser eksternal.",
            host
        )));
    }

    Ok(parsed)
}

#[tauri::command]
pub fn shell_open_external(app: tauri::AppHandle, url: String) -> Result<(), LauncherErrorPayload> {
    validate_external_url(&url)?;

    app.opener().open_url(&url, None::<&str>).map_err(|e| {
        LauncherErrorPayload::internal(format!("Gagal membuka URL di sistem operasi: {}", e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_open_external_allowlist() {
        assert!(validate_external_url("https://modrinth.com/mod/sodium").is_ok());
        assert!(validate_external_url("https://subdomain.minecraft.net/page").is_ok());
        assert!(validate_external_url("https://curseforge.com/minecraft").is_ok());

        assert!(validate_external_url("http://modrinth.com/mod").is_err());

        assert!(validate_external_url("https://evilmodrinth.com").is_err());
        assert!(validate_external_url("https://notminecraft.net").is_err());
        assert!(validate_external_url("https://attacker.com").is_err());

        assert!(validate_external_url("not-a-url").is_err());
        assert!(validate_external_url("file:///etc/passwd").is_err());
    }
}
