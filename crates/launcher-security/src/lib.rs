use launcher_types::LauncherErrorPayload;
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};

const MAX_SEGMENT_BYTES: usize = 255;
const MAX_REL_PATH_LEN: usize = 1024;
const MAX_REL_PATH_DEPTH: usize = 64;

fn normalize_reserved_stem(stem: &str) -> String {
    let trimmed = stem.trim_end_matches([' ', '.']);
    let mut out = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        match c {
            '\u{00B9}' => out.push('1'),
            '\u{00B2}' => out.push('2'),
            '\u{00B3}' => out.push('3'),
            _ => out.push(c.to_ascii_lowercase()),
        }
    }
    out
}

/// SEC-09: Penolakan nama perangkat DOS cadangan Windows pada instance, world, dan server.
/// Mencakup CON, PRN, AUX, NUL, COM1-9, LPT1-9 baik berdiri sendiri, berekstensi (mis. CON.txt),
/// varian superskrip Unicode (COM¹⁻³, LPT¹⁻³), spasi/titik sebelum ekstensi (mis. "CON .txt"),
/// maupun Alternate Data Stream NTFS (mis. AUX:$DATA atau CON:stream).
pub fn is_windows_reserved_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }

    let raw_stem = trimmed.split(['.', ':']).next().unwrap_or(trimmed);
    let stem = normalize_reserved_stem(raw_stem);

    matches!(
        stem.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    )
}

fn has_forbidden_segment_char(segment: &str) -> bool {
    segment.chars().any(|c| {
        c == '\0'
            || c < ' '
            || c == '\u{007f}'
            || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')
    })
}

/// Memvalidasi satu segmen nama berkas atau direktori lintas platform (Windows & Linux).
pub fn validate_path_segment(segment: &str) -> Result<&str, LauncherErrorPayload> {
    if segment.is_empty() {
        return Err(LauncherErrorPayload::invalid_input(
            "Segmen path tidak boleh kosong.",
        ));
    }
    if segment == "." || segment == ".." {
        return Err(LauncherErrorPayload::invalid_input(
            "Segmen '.' atau '..' tidak diizinkan dalam path.",
        ));
    }
    if segment.len() > MAX_SEGMENT_BYTES {
        return Err(LauncherErrorPayload::invalid_input(
            "Panjang nama berkas atau folder melebihi batas 255 byte.",
        ));
    }
    if segment.starts_with(' ') || segment.ends_with(' ') || segment.ends_with('.') {
        return Err(LauncherErrorPayload::invalid_input(
            "Nama berkas atau folder tidak boleh diawali spasi atau diakhiri spasi/titik.",
        ));
    }
    if has_forbidden_segment_char(segment) {
        return Err(LauncherErrorPayload::invalid_input(
            "Nama berkas atau folder memuat karakter terlarang.",
        ));
    }
    if is_windows_reserved_name(segment) {
        return Err(LauncherErrorPayload::invalid_input(
            "Nama berkas atau folder menggunakan nama perangkat reserved Windows (SEC-09).",
        ));
    }
    Ok(segment)
}

/// Alias semantik untuk memvalidasi satu nama berkas (tanpa separator maupun traversal).
pub fn validate_single_filename(name: &str) -> Result<&str, LauncherErrorPayload> {
    validate_path_segment(name)
}

/// Memvalidasi satu ID segmen tunggal (mis. version_id, index_id, component, profile_id).
pub fn validate_safe_single_component(id: &str) -> Result<&str, LauncherErrorPayload> {
    validate_path_segment(id)
}

/// Wrapper path relatif tervalidasi agar caller tidak sembarangan menggabung string mentah ke Path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafeRelPath(PathBuf);

impl SafeRelPath {
    /// Mem-parse dan memvalidasi path relatif lintas-OS (memakai '/' dan menolak '\', '..', '.', ':', reserved name).
    pub fn parse(rel_path: &str) -> Result<Self, LauncherErrorPayload> {
        if rel_path.is_empty() || rel_path.trim().is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif tidak boleh kosong.",
            ));
        }
        if rel_path != rel_path.trim() {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif tidak boleh memiliki spasi di awal atau akhir.",
            ));
        }
        if rel_path.len() > MAX_REL_PATH_LEN {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif melebihi batas panjang maksimum.",
            ));
        }
        if rel_path.contains('\0') || rel_path.contains('\\') {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif memuat backslash atau karakter NUL yang tidak diizinkan.",
            ));
        }
        if rel_path.starts_with('/') || Path::new(rel_path).is_absolute() {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif tidak boleh absolut.",
            ));
        }

        let mut clean = PathBuf::new();
        let mut depth = 0usize;
        for part in rel_path.split('/') {
            depth += 1;
            if depth > MAX_REL_PATH_DEPTH {
                return Err(LauncherErrorPayload::invalid_input(
                    "Kedalaman direktori path melebihi batas maksimum.",
                ));
            }
            validate_path_segment(part)?;
            clean.push(part);
        }

        if depth == 0 {
            return Err(LauncherErrorPayload::invalid_input(
                "Path relatif tidak memiliki segmen valid.",
            ));
        }

        Ok(Self(clean))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Menggabungkan ke `base` dan memverifikasi tidak ada komponen symlink/reparse point pada `base` maupun turunannya.
    pub fn resolve_under(&self, base: &Path) -> Result<PathBuf, LauncherErrorPayload> {
        let joined = base.join(&self.0);
        ensure_descendants_no_symlink(base, &joined)?;
        Ok(joined)
    }
}

/// SEC-01: Memastikan subpath relatif aman dari eksploitasi traversal ('..', '.', drive letters, root, ADS, reserved names).
pub fn safe_subpath(base: &Path, rel_path: &str) -> Result<PathBuf, LauncherErrorPayload> {
    let safe_rel = SafeRelPath::parse(rel_path)?;
    Ok(base.join(safe_rel.as_path()))
}

/// Memeriksa secara komponen (bukan string prefix) apakah `path` berada di dalam `root` setelah kanonikalisasi.
pub fn canonical_starts_with(path: &Path, root: &Path) -> Result<bool, LauncherErrorPayload> {
    let c_root = dunce::canonicalize(root).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal kanonikalisasi root: {}", e))
    })?;
    let c_path = dunce::canonicalize(path).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal kanonikalisasi path: {}", e))
    })?;
    Ok(c_path.starts_with(&c_root))
}

fn check_not_symlink_if_exists(path: &Path) -> Result<bool, LauncherErrorPayload> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Operasi ditolak: terdeteksi symlink atau junction pada komponen path.",
                ));
            }
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(LauncherErrorPayload::file_system(format!(
            "Gagal memeriksa metadata path: {}",
            e
        ))),
    }
}

/// Memverifikasi `root` (jika ada) serta seluruh komponen turunan yang sudah ada di disk hingga `target`
/// bukanlah symlink maupun NTFS junction/reparse point.
pub fn ensure_descendants_no_symlink(
    root: &Path,
    target: &Path,
) -> Result<(), LauncherErrorPayload> {
    let rel = target.strip_prefix(root).map_err(|_| {
        LauncherErrorPayload::invalid_input("Target path berada di luar direktori root.")
    })?;

    check_not_symlink_if_exists(root)?;
    let mut curr = root.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => {
                curr.push(part);
                if !check_not_symlink_if_exists(&curr)? {
                    continue;
                }
            }
            Component::CurDir => {}
            _ => {
                return Err(LauncherErrorPayload::invalid_input(
                    "Komponen path tidak valid di dalam root.",
                ));
            }
        }
    }

    if root.exists() && target.exists() && !canonical_starts_with(target, root)? {
        return Err(LauncherErrorPayload::invalid_input(
            "Target path keluar dari batas direktori root.",
        ));
    }

    Ok(())
}

/// Membuat direktori (dan mengatur izin `0o700` di Unix jika `private_unix` aktif) dengan validasi anti-symlink terhadap `root`.
pub fn ensure_dir_within(
    root: &Path,
    target: &Path,
    private_unix: bool,
) -> Result<PathBuf, LauncherErrorPayload> {
    ensure_descendants_no_symlink(root, target)?;
    ensure_secure_dir(target, private_unix)?;
    if !canonical_starts_with(target, root)? {
        return Err(LauncherErrorPayload::invalid_input(
            "Direktori berada di luar root yang diizinkan.",
        ));
    }
    Ok(target.to_path_buf())
}

/// Membuat direktori dengan mode `0o700` di Unix apabila `private_unix` bernilai `true`.
pub fn ensure_secure_dir(dir: &Path, private_unix: bool) -> Result<(), LauncherErrorPayload> {
    fs::create_dir_all(dir).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membuat direktori: {}", e))
    })?;

    #[cfg(unix)]
    if private_unix {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::symlink_metadata(dir).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal memeriksa direktori: {}", e))
        })?;
        if meta.file_type().is_symlink() {
            return Err(LauncherErrorPayload::invalid_input(
                "Direktori target tidak boleh berupa symlink.",
            ));
        }
        let perms = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(dir, perms);
    }

    #[cfg(not(unix))]
    let _ = private_unix;

    Ok(())
}

/// Membuka berkas baru secara eksklusif (`create_new(true)`) tanpa mengikuti symlink yang mungkin sudah ada.
pub fn safe_create_new_file(path: &Path, private_unix: bool) -> Result<File, LauncherErrorPayload> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);

    #[cfg(unix)]
    if private_unix {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    #[cfg(not(unix))]
    let _ = private_unix;

    opts.open(path).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membuat berkas secara aman: {}", e))
    })
}

/// Menghapus direktori target secara rekursif hanya bila ia turunan ketat `root` dan bebas symlink/junction.
pub fn safe_remove_dir_all_within(root: &Path, target: &Path) -> Result<(), LauncherErrorPayload> {
    if target == root {
        return Err(LauncherErrorPayload::invalid_input(
            "Operasi penghapusan direktori root ditolak.",
        ));
    }
    ensure_descendants_no_symlink(root, target)?;
    match fs::symlink_metadata(target) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Menolak menghapus symlink sebagai direktori target.",
                ));
            }
            if !meta.file_type().is_dir() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target penghapusan bukan direktori.",
                ));
            }
            if root.exists() && !canonical_starts_with(target, root)? {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target penghapusan berada di luar root.",
                ));
            }
            fs::remove_dir_all(target).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal menghapus direktori: {}", e))
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(LauncherErrorPayload::file_system(format!(
            "Gagal memeriksa direktori sebelum hapus: {}",
            e
        ))),
    }
}

/// Menghapus satu berkas (bukan direktori) di dalam `root` setelah memeriksa batas & bebas symlink pada induk maupun target.
pub fn safe_remove_file_within(root: &Path, target: &Path) -> Result<(), LauncherErrorPayload> {
    if target == root {
        return Err(LauncherErrorPayload::invalid_input(
            "Target penghapusan berkas tidak valid.",
        ));
    }
    ensure_descendants_no_symlink(root, target)?;
    match fs::symlink_metadata(target) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || meta.file_type().is_dir() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target penghapusan berkas bukan berkas reguler.",
                ));
            }
            fs::remove_file(target).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal menghapus berkas: {}", e))
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(LauncherErrorPayload::file_system(format!(
            "Gagal memeriksa berkas sebelum hapus: {}",
            e
        ))),
    }
}

/// Memvalidasi target symlink relatif di dalam arsip/manifes Unix (membolehkan `../` internal asalkan tidak keluar dari `root`).
pub fn validate_relative_symlink_target_within(
    root: &Path,
    link_path: &Path,
    target: &str,
) -> Result<(), LauncherErrorPayload> {
    if target.is_empty() || target.contains('\0') || target.contains('\\') {
        return Err(LauncherErrorPayload::invalid_input(
            "Target symlink tidak valid.",
        ));
    }
    if target.starts_with('/') || Path::new(target).is_absolute() {
        return Err(LauncherErrorPayload::invalid_input(
            "Target symlink absolut ditolak.",
        ));
    }

    let rel_link = link_path.strip_prefix(root).map_err(|_| {
        LauncherErrorPayload::invalid_input("Lokasi symlink berada di luar direktori target.")
    })?;

    let mut stack: Vec<String> = rel_link
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    Component::Normal(s) => s.to_str().map(|v| v.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    for seg in target.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            if stack.pop().is_none() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target symlink mencoba keluar dari root direktori.",
                ));
            }
        } else {
            validate_path_segment(seg)?;
            stack.push(seg.to_string());
        }
    }

    Ok(())
}

/// Mengkanonisasi path meskipun ujungnya belum ada di disk dengan mengkanonisasi
/// leluhur terdalam yang ada lalu menyambungkan sisa komponen relatifnya.
pub fn canonicalize_with_tail(path: &Path) -> PathBuf {
    if let Ok(canon) = dunce::canonicalize(path) {
        return canon;
    }

    let mut tail_rev: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path;
    while fs::symlink_metadata(cursor).is_err() {
        let Some(name) = cursor.file_name() else {
            break;
        };
        tail_rev.push(name.to_os_string());
        let Some(parent) = cursor.parent() else {
            break;
        };
        cursor = parent;
    }

    let mut base = if fs::symlink_metadata(cursor).is_ok() {
        match dunce::canonicalize(cursor) {
            Ok(canon) => canon,
            // Broken/dangling symlink at `cursor`: return the original path rather than skipping over it.
            Err(_) => return path.to_path_buf(),
        }
    } else {
        return path.to_path_buf();
    };
    for seg in tail_rev.into_iter().rev() {
        base.push(seg);
    }
    base
}

fn ensure_disk_or_unix_path(
    path: &Path,
    reject_parent_dir: bool,
) -> Result<(), LauncherErrorPayload> {
    for comp in path.components() {
        match comp {
            Component::ParentDir if reject_parent_dir => {
                return Err(LauncherErrorPayload::invalid_input(
                    "Path tidak boleh mengandung komponen '..'.",
                ));
            }
            Component::Prefix(prefix) if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {
                return Err(LauncherErrorPayload::invalid_input(
                    "Path UNC, Verbatim, atau device Windows tidak diizinkan.",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Memvalidasi path kandidat root data launcher (`MCLAUNCHER_DATA_ROOT` atau resolver env).
pub fn validate_data_root_candidate(path: &Path) -> Result<(), LauncherErrorPayload> {
    let raw = path.to_string_lossy();
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err(LauncherErrorPayload::invalid_input(
            "Root data launcher tidak boleh kosong atau memuat karakter NUL.",
        ));
    }
    if !path.is_absolute() {
        return Err(LauncherErrorPayload::invalid_input(
            "Root data launcher harus berupa path absolut.",
        ));
    }

    ensure_disk_or_unix_path(path, true)?;

    if path.parent().is_none() {
        return Err(LauncherErrorPayload::invalid_input(
            "Root data launcher tidak boleh menunjuk ke root drive atau root filesystem.",
        ));
    }

    #[cfg(windows)]
    {
        let resolved = canonicalize_with_tail(path);
        ensure_disk_or_unix_path(&resolved, true)?;
        let normalized = resolved
            .to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase();

        let mut sys_prefixes: Vec<String> = vec![
            "c:\\windows".to_string(),
            "c:\\program files".to_string(),
            "c:\\program files (x86)".to_string(),
            "c:\\programdata".to_string(),
        ];
        for var in [
            "SystemRoot",
            "WINDIR",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
        ] {
            if let Ok(val) = std::env::var(var) {
                let norm = dunce::canonicalize(&val)
                    .unwrap_or_else(|_| PathBuf::from(val))
                    .to_string_lossy()
                    .replace('/', "\\")
                    .trim_end_matches('\\')
                    .to_ascii_lowercase();
                if !norm.is_empty() && !sys_prefixes.contains(&norm) {
                    sys_prefixes.push(norm);
                }
            }
        }
        for sys in sys_prefixes {
            if normalized == sys || normalized.starts_with(&format!("{}\\", sys)) {
                return Err(LauncherErrorPayload::invalid_input(
                    "Root data launcher tidak boleh berada di direktori sistem Windows.",
                ));
            }
        }
    }

    #[cfg(not(windows))]
    {
        let normalized = raw.trim_end_matches('/');
        let forbidden = [
            "/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/opt", "/proc", "/root",
            "/run", "/sbin", "/sys", "/usr", "/var",
        ];
        if forbidden.contains(&normalized) {
            return Err(LauncherErrorPayload::invalid_input(
                "Root data launcher tidak boleh menunjuk langsung ke direktori sistem Unix.",
            ));
        }
    }

    Ok(())
}

/// SEC-03: Memvalidasi executable Java sebelum dijalankan lewat `Command::new`.
pub fn validate_java_executable_path(path: &Path) -> Result<PathBuf, LauncherErrorPayload> {
    if !path.is_absolute() {
        return Err(LauncherErrorPayload::invalid_input(
            "Path executable Java harus absolut.",
        ));
    }
    // Reject UNC/device/verbatim prefixes and traversal before canonicalize triggers SMB probes.
    ensure_disk_or_unix_path(path, true)?;

    let check_java_name = |p: &Path| -> bool {
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        if cfg!(windows) {
            name.eq_ignore_ascii_case("java.exe") || name.eq_ignore_ascii_case("javaw.exe")
        } else {
            name == "java"
        }
    };

    if !check_java_name(path) {
        return Err(LauncherErrorPayload::invalid_input(
            "Executable Java harus bernama java/java.exe/javaw.exe.",
        ));
    }

    let canonical = dunce::canonicalize(path).map_err(|e| {
        LauncherErrorPayload::not_found(format!(
            "Berkas Java '{}' tidak ditemukan atau tidak valid: {}",
            path.display(),
            e
        ))
    })?;
    ensure_disk_or_unix_path(&canonical, true)?;

    if !check_java_name(&canonical) {
        return Err(LauncherErrorPayload::invalid_input(
            "Target kanonikal executable bukan biner Java yang sah.",
        ));
    }

    let meta = fs::metadata(&canonical).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membaca metadata berkas Java: {}", e))
    })?;
    if !meta.is_file() {
        return Err(LauncherErrorPayload::invalid_input(
            "Target Java harus berupa berkas reguler.",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = canonical.parent() {
            if let Ok(pmeta) = fs::metadata(parent) {
                if pmeta.permissions().mode() & 0o002 != 0 {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Direktori executable Java dapat ditulisi semua pengguna (world-writable).",
                    ));
                }
            }
        }
    }

    Ok(canonical)
}

/// Memvalidasi format ID instance agar aman digunakan sebagai folder sistem berkas.
pub fn validate_instance_id(id: &str) -> Result<(), LauncherErrorPayload> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance tidak boleh kosong.",
        ));
    }

    if id != trimmed {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance tidak boleh memiliki spasi di awal atau akhir.",
        ));
    }

    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance tidak boleh berawalan atau berakhiran titik ('.').",
        ));
    }

    if trimmed == "." || trimmed == ".." {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance tidak boleh berupa '.' atau '..'",
        ));
    }

    if trimmed.len() > 128 {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance maksimal 128 karakter.",
        ));
    }

    let is_valid_chars = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');

    if !is_valid_chars {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance hanya boleh memuat huruf, angka, titik, underscore, dan tanda hubung (-).",
        ));
    }

    if is_windows_reserved_name(trimmed) {
        return Err(LauncherErrorPayload::invalid_input(
            "ID instance tidak boleh menggunakan nama perangkat reserved Windows (SEC-09).",
        ));
    }

    Ok(())
}

/// Menghasilkan slug aman dari nama instance yang dimasukkan pengguna.
pub fn slugify_instance_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut last_dash = false;

    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if (c == ' ' || c == '-' || c == '_') && !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }

    let capped = if slug.len() > 128 {
        &slug[..128]
    } else {
        slug.as_str()
    };
    let trimmed = capped.trim_end_matches('-');
    if trimmed.is_empty() || is_windows_reserved_name(trimmed) {
        format!("instance-{}", uuid::Uuid::new_v4().simple())
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_reserved_names() {
        assert!(is_windows_reserved_name("CON"));
        assert!(is_windows_reserved_name("con"));
        assert!(is_windows_reserved_name("prn"));
        assert!(is_windows_reserved_name("AUX.txt"));
        assert!(is_windows_reserved_name("NUL.tar.gz"));
        assert!(is_windows_reserved_name("com1"));
        assert!(is_windows_reserved_name("LPT9.log"));
        assert!(is_windows_reserved_name("AUX:$DATA"));
        assert!(is_windows_reserved_name("con:stream"));

        assert!(!is_windows_reserved_name("container"));
        assert!(!is_windows_reserved_name("my-instance"));
        assert!(!is_windows_reserved_name("world1"));
    }

    #[test]
    fn test_validate_instance_id() {
        assert!(validate_instance_id("vanilla-1-21").is_ok());
        assert!(validate_instance_id("my_modpack_1").is_ok());

        assert!(validate_instance_id("").is_err());
        assert!(validate_instance_id(".").is_err());
        assert!(validate_instance_id("..").is_err());
        assert!(validate_instance_id(".hidden").is_err());
        assert!(validate_instance_id("trailing_dot.").is_err());
        assert!(validate_instance_id(" space_prefix").is_err());
        assert!(validate_instance_id("con").is_err());
        assert!(validate_instance_id("folder/name").is_err());
        assert!(validate_instance_id("instance with spaces").is_err());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify_instance_name("Fabric 1.21.1"), "fabric-1211");
        assert_eq!(
            slugify_instance_name("My Super Modpack!"),
            "my-super-modpack"
        );
        assert!(slugify_instance_name("CON").starts_with("instance-"));
    }

    #[test]
    fn test_safe_subpath() {
        let base = Path::new("C:\\data\\instances\\inst-1\\minecraft");
        assert!(safe_subpath(base, "mods/jei.jar").is_ok());
        assert!(safe_subpath(base, "config/sub/conf.json").is_ok());

        assert!(safe_subpath(base, "../evil.exe").is_err());
        assert!(safe_subpath(base, "mods/../../evil.exe").is_err());
        assert!(safe_subpath(base, "/etc/passwd").is_err());
        assert!(safe_subpath(base, "C:\\windows\\system32").is_err());
    }

    #[test]
    fn test_ps_windows_reserved_and_superscript_variants() {
        assert!(is_windows_reserved_name("COM¹"));
        assert!(is_windows_reserved_name("LPT²"));
        assert!(is_windows_reserved_name("LPT³.txt"));
        assert!(is_windows_reserved_name("CON .txt"));
        assert!(validate_single_filename("COM¹.dll").is_err());
        assert!(validate_single_filename("CON .txt").is_err());
        assert!(validate_single_filename("trailing.").is_err());
        assert!(validate_single_filename("trailing ").is_err());
        assert!(validate_single_filename(" leading").is_err());
        assert!(validate_single_filename("ads:stream").is_err());
        assert!(validate_single_filename("bad\0name").is_err());
        assert!(validate_single_filename("mod.jar").is_ok());
    }

    #[test]
    fn test_ps_safe_subpath_rejects_drive_relative_unc_and_root_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("root");
        let sibling_evil = temp.path().join("root-evil");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&sibling_evil).unwrap();

        assert!(safe_subpath(&base, "").is_err());
        assert!(safe_subpath(&base, ".").is_err());
        assert!(safe_subpath(&base, "C:relative").is_err());
        assert!(safe_subpath(&base, "\\root_rel").is_err());
        assert!(safe_subpath(&base, "\\\\server\\share\\file").is_err());
        assert!(!canonical_starts_with(&sibling_evil, &base).unwrap());
        assert!(safe_remove_dir_all_within(&base, &sibling_evil).is_err());
        assert!(safe_remove_dir_all_within(&base, &base).is_err());
    }

    #[test]
    fn test_ps_data_root_and_java_validation() {
        let temp = tempfile::tempdir().unwrap();
        assert!(validate_data_root_candidate(Path::new("relative/root")).is_err());
        assert!(validate_data_root_candidate(Path::new("\\\\server\\share\\mc")).is_err());
        #[cfg(windows)]
        {
            assert!(validate_data_root_candidate(Path::new("C:\\")).is_err());
            assert!(validate_data_root_candidate(Path::new("C:\\Windows\\Temp\\mc")).is_err());
            assert!(
                validate_data_root_candidate(Path::new("\\\\?\\C:\\Windows\\Temp\\mc")).is_err()
            );
            assert!(
                validate_data_root_candidate(Path::new("C:\\Users\\test\\..\\Windows")).is_err()
            );
            assert!(
                validate_java_executable_path(Path::new("\\\\attacker\\share\\java.exe")).is_err()
            );
        }
        assert!(validate_data_root_candidate(&temp.path().join("mc-root")).is_ok());

        let long_name = "a".repeat(200);
        let slug = slugify_instance_name(&long_name);
        assert!(slug.len() <= 128);
        assert!(validate_instance_id(&slug).is_ok());

        let fake_cmd = temp.path().join("fake.cmd");
        fs::write(&fake_cmd, b"@echo off\r\n").unwrap();
        assert!(validate_java_executable_path(&fake_cmd).is_err());
        assert!(validate_java_executable_path(Path::new("java.exe")).is_err());
        let dir_named_java = temp
            .path()
            .join(if cfg!(windows) { "java.exe" } else { "java" });
        fs::create_dir_all(&dir_named_java).unwrap();
        assert!(validate_java_executable_path(&dir_named_java).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn test_ps_windows_junction_rejected_by_remove_and_ensure_dir() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside-secret");
        let junction = root.join("link-out");
        let keep_file = outside.join("keep.txt");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(&keep_file, b"important").unwrap();

        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                outside.to_str().unwrap(),
            ])
            .status()
            .expect("cmd mklink /J");
        assert!(status.success(), "mklink /J must succeed in temp dir");

        assert!(ensure_descendants_no_symlink(&root, &junction.join("sub")).is_err());
        assert!(ensure_dir_within(&root, &junction.join("sub"), false).is_err());
        assert!(safe_remove_dir_all_within(&root, &junction).is_err());
        assert!(
            keep_file.exists(),
            "Junction target contents must NOT be deleted!"
        );
    }
}
