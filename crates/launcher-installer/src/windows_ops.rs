use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn default_install_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            return PathBuf::from(local_app_data)
                .join("Programs")
                .join("MSE Launcher");
        }
    }
    PathBuf::from(r"C:\MSE Launcher")
}

pub fn default_portable_dir() -> PathBuf {
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        if !user_profile.trim().is_empty() {
            return PathBuf::from(user_profile)
                .join("Desktop")
                .join("MSE Launcher Portable");
        }
    }
    PathBuf::from(r"C:\MSE Launcher Portable")
}

pub fn user_data_dir() -> Option<PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| PathBuf::from(v).join("MCLauncher"))
}

pub fn desktop_shortcut_path() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| PathBuf::from(v).join("Desktop").join("MSE Launcher.lnk"))
}

pub fn start_menu_shortcut_path() -> Option<PathBuf> {
    std::env::var("APPDATA")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            PathBuf::from(v)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("MSE Launcher.lnk")
        })
}

pub fn detect_system_language() -> String {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName",
            ])
            .output();
        if let Ok(out) = output {
            let lang = String::from_utf8_lossy(&out.stdout)
                .trim()
                .to_ascii_lowercase();
            if lang == "id" || lang == "ms" {
                return "id".to_string();
            }
            if !lang.is_empty() {
                return "en".to_string();
            }
        }
    }
    "en".to_string()
}

pub fn pick_folder_dialog(initial_path: Option<&str>, title: &str) -> Option<String> {
    #[cfg(windows)]
    {
        let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Application]::EnableVisualStyles()
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = $env:MSE_PICK_TITLE
$dlg.UseDescriptionForTitle = $true
$dlg.ShowNewFolderButton = $true
if ($env:MSE_PICK_INITIAL -and (Test-Path -LiteralPath $env:MSE_PICK_INITIAL)) {
    $dlg.SelectedPath = $env:MSE_PICK_INITIAL
}
$owner = New-Object System.Windows.Forms.Form
$owner.TopMost = $true
$res = $dlg.ShowDialog($owner)
$owner.Dispose()
if ($res -eq [System.Windows.Forms.DialogResult]::OK) {
    [Console]::Out.Write($dlg.SelectedPath)
}
"#;
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.creation_flags(CREATE_NO_WINDOW)
            .env("MSE_PICK_TITLE", title)
            .env("MSE_PICK_INITIAL", initial_path.unwrap_or(""))
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ]);
        let output = cmd.output().ok()?;
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            None
        } else {
            Some(selected)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (initial_path, title);
        None
    }
}

pub fn close_running_launcher() {
    #[cfg(windows)]
    {
        let script = r#"
$names = @('MSE Launcher', 'minecraft-launcher-desktop')
for ($i = 0; $i -lt 10; $i++) {
    $procs = Get-Process -Name $names -ErrorAction SilentlyContinue
    if (-not $procs) { break }
    if ($i -eq 0) {
        $procs | ForEach-Object { $_.CloseMainWindow() | Out-Null }
    } elseif ($i -ge 3) {
        $procs | Stop-Process -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 250
}
"#;
        let _ = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .status();
    }
}

pub fn create_shortcut(
    target_exe: &Path,
    shortcut_path: &Path,
    description: &str,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        if let Some(parent) = shortcut_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let work_dir = target_exe
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let script = r#"
$wsh = New-Object -ComObject WScript.Shell
$sc = $wsh.CreateShortcut($env:MSE_SC_PATH)
$sc.TargetPath = $env:MSE_SC_TARGET
$sc.WorkingDirectory = $env:MSE_SC_WORKDIR
$sc.IconLocation = "$env:MSE_SC_TARGET,0"
$sc.Description = $env:MSE_SC_DESC
$sc.Save()
"#;
        let status = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .env("MSE_SC_PATH", shortcut_path)
            .env("MSE_SC_TARGET", target_exe)
            .env("MSE_SC_WORKDIR", work_dir)
            .env("MSE_SC_DESC", description)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to create Windows shortcut".to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (target_exe, shortcut_path, description);
        Ok(())
    }
}

pub fn register_uninstall_entry(
    install_dir: &Path,
    exe_path: &Path,
    uninstaller_path: &Path,
    version: &str,
    estimated_size_kb: u64,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let uninstall_cmd = format!("\"{}\" --uninstall", uninstaller_path.display());
        let script = r#"
$regPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MSELauncher'
if (-not (Test-Path $regPath)) {
    New-Item -Path $regPath -Force | Out-Null
}
New-ItemProperty -Path $regPath -Name 'DisplayName' -Value 'MSE Launcher' -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'DisplayVersion' -Value $env:MSE_REG_VER -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'Publisher' -Value 'MSE Launcher Contributors' -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'InstallLocation' -Value $env:MSE_REG_DIR -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'DisplayIcon' -Value $env:MSE_REG_ICON -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'UninstallString' -Value $env:MSE_REG_UNINST -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'QuietUninstallString' -Value $env:MSE_REG_UNINST -PropertyType String -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'EstimatedSize' -Value ([int]$env:MSE_REG_SIZE) -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'NoModify' -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $regPath -Name 'NoRepair' -Value 1 -PropertyType DWord -Force | Out-Null
"#;
        let status = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .env("MSE_REG_VER", version)
            .env("MSE_REG_DIR", install_dir)
            .env("MSE_REG_ICON", exe_path)
            .env("MSE_REG_UNINST", uninstall_cmd)
            .env("MSE_REG_SIZE", estimated_size_kb.to_string())
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to register uninstall entry".to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (
            install_dir,
            exe_path,
            uninstaller_path,
            version,
            estimated_size_kb,
        );
        Ok(())
    }
}

pub fn unregister_uninstall_entry() {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "Remove-Item -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MSELauncher' -Force -ErrorAction SilentlyContinue",
            ])
            .status();
    }
}

pub fn read_registered_install_location() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MSELauncher' -ErrorAction SilentlyContinue).InstallLocation | ForEach-Object { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::Out.Write($_) }",
            ])
            .output()
            .ok()?;
        let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if val.is_empty() {
            None
        } else {
            Some(PathBuf::from(val))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn launch_executable(exe_path: &Path) -> Result<(), String> {
    std::process::Command::new(exe_path)
        .spawn()
        .map_err(|e| format!("Failed to launch {}: {e}", exe_path.display()))?;
    Ok(())
}

pub fn schedule_self_cleanup(uninstaller_exe: &Path, install_dir: &Path) {
    #[cfg(windows)]
    {
        let pid = std::process::id().to_string();
        let script = r#"
$pidVal = [int]$env:MSE_UNINST_PID
for ($i = 0; $i -lt 40; $i++) {
    if (-not (Get-Process -Id $pidVal -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Milliseconds 250
}
Remove-Item -LiteralPath $env:MSE_UNINST_EXE -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $env:MSE_UNINST_DIR) {
    $items = Get-ChildItem -LiteralPath $env:MSE_UNINST_DIR -Force -ErrorAction SilentlyContinue
    if (-not $items) {
        Remove-Item -LiteralPath $env:MSE_UNINST_DIR -Force -ErrorAction SilentlyContinue
    }
}
"#;
        let _ = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .env("MSE_UNINST_PID", pid)
            .env("MSE_UNINST_EXE", uninstaller_exe)
            .env("MSE_UNINST_DIR", install_dir)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = (uninstaller_exe, install_dir);
    }
}
