# MSE Launcher

> **Disclaimer:** NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
> *(Bukan produk resmi Minecraft. Tidak disetujui oleh atau berafiliasi dengan Mojang Synergies AB maupun Microsoft Corporation.)*

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20x64%20%7C%20Linux%20x64-0078D6.svg)](#development)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg?logo=rust)](rust-toolchain.toml)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB.svg?logo=tauri)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB.svg?logo=react)](https://react.dev/)

> [!WARNING]
> **AI / Vibe Coding Notice:** This application was built using an **AI**.

**[English](#english)** · **[Bahasa Indonesia](#bahasa-indonesia)** · **[Version History / Riwayat Versi](#version-history--riwayat-versi)** · **[Development](#development)**

---

## English

A lightweight desktop Minecraft launcher and instance manager built with **Tauri v2**, **Rust**, and **React 19** for Windows x64 and Linux x64.

### Download & Choose Your Edition (Windows x64)
Visit the **[Releases](https://github.com/IrfanSyahid118/MSE-Launcher/releases/latest)** page and choose the file that fits how you want to run MSE Launcher:
- **Standard Installer (`*_x64-setup.exe`) — Recommended:** Installs MSE Launcher normally on Windows with Start Menu / Desktop shortcuts.
- **MSI Installer (`*_x64_en-US.msi`):** Alternative Microsoft Windows Installer package.
- **Portable Edition (`*_Portable.exe`):** Run directly from any folder or USB drive without installing.
- *(Ignore `latest.json` — it is only read automatically by the in-app updater.)*

### Key Features
- **Version & Mod Loader Management:** Supports Vanilla releases, snapshots, beta/alpha versions, plus **Fabric**, **Quilt**, **NeoForge**, and **Forge** installers.
- **Mod & Modpack Browser:** Search and install mods, modpacks (`.mrpack` and CurseForge `.zip`), resource packs, shader packs, and worlds from **Modrinth** and **CurseForge** with SHA-1 integrity verification.
- **Microsoft & Offline Accounts + 3D Skin Viewer:** Sign in with Microsoft OAuth or create offline profiles, complete with an interactive WebGL 3D character preview (Classic 4px and Slim 3px models) and custom skin uploads.
- **Built-in Auto-Updater (`v1.2.1-beta`):** Check for updates directly inside the launcher. It automatically detects whether you are running the **Portable** or **Installer** edition, downloads the matching binary, verifies its SHA-256 checksum, and applies the update.
- **Isolated Instances & Java Runtime Management:** Keep separate game directories per profile, automatically resolve Mojang Java runtimes (`jre-legacy`, `java-runtime-*`) or system Java, tune RAM limits and JVM presets, manage `servers.dat`, browse screenshots, and inspect live console/diagnostic bundles (`MCSE1:...`).
- **Optional Discord Rich Presence:** Displays idle or in-game instance status via local Discord IPC (can be toggled off in Settings).
- **Security Controls:** Windows DPAPI and Linux AES-256-GCM (`0600` key) encrypted token storage (`secrets.bin`), heap buffer `zeroize`, atomic JSON persistence with corruption quarantine, path traversal/symlink guards, and ZIP extraction quotas. See [SECURITY.md](SECURITY.md) for the full threat model.

---

## Bahasa Indonesia

Launcher Minecraft desktop dan pengelola instance yang ringan dan modern, dibangun menggunakan **Tauri v2**, **Rust**, dan **React 19** untuk Windows x64 dan Linux x64.

### Unduh & Pilih Jenis Aplikasi (Windows x64)
Buka halaman **[Releases](https://github.com/IrfanSyahid118/MSE-Launcher/releases/latest)** lalu pilih file sesuai kebutuhanmu:
- **Installer Standar (`*_x64-setup.exe`) — Direkomendasikan:** Memasang MSE Launcher seperti aplikasi Windows biasa lengkap dengan pintasan Start Menu / Desktop.
- **MSI Installer (`*_x64_en-US.msi`):** Alternatif paket instalasi format Microsoft Windows Installer.
- **Versi Portable (`*_Portable.exe`):** Langsung jalankan dari folder mana saja atau flashdisk tanpa perlu instalasi.
- *(File `latest.json` tidak perlu diunduh manual — file ini dipakai otomatis oleh fitur auto-updater di dalam aplikasi.)*

### Fitur Utama
- **Pengelola Versi & Mod Loader:** Mendukung Minecraft Vanilla (Release, Snapshot, Beta/Alpha) serta instalasi otomatis **Fabric**, **Quilt**, **NeoForge**, dan **Forge**.
- **Browser Mod & Modpack:** Cari dan pasang mod, modpack (`.mrpack` dan CurseForge `.zip`), resource pack, shader pack, serta world langsung dari **Modrinth** dan **CurseForge** lengkap dengan verifikasi integritas SHA-1.
- **Akun Microsoft & Offline + 3D Skin Viewer:** Login dengan akun Microsoft atau gunakan profil offline, dilengkapi pratinjau karakter 3D interaktif (model Classic 4px & Slim 3px) dan unggah skin kustom.
- **Auto-Updater Bawaan (`v1.2.1-beta`):** Cek dan pasang pembaruan langsung dari launcher (sidebar bawah / Settings). Launcher otomatis mengenali apakah kamu memakai versi **Portable** atau **Installer**, memverifikasi SHA-256, lalu memperbarui aplikasi secara otomatis.
- **Instance Terisolasi & Pengaturan Java:** Pisahkan folder game per instance, gunakan Java bawaan Mojang (`jre-legacy`, `java-runtime-*`) atau Java sistem, atur batas RAM & preset JVM, kelola daftar server multiplayer (`servers.dat`), buka galeri screenshot, serta pantau konsol live & diagnostic bundle (`MCSE1:...`).
- **Discord Rich Presence (Opsional):** Menampilkan status sedang di menu atau bermain instance tertentu di Discord lokal (bisa dimatikan di Settings).
- **Keamanan Data:** Penyimpanan token terenkripsi (Windows DPAPI & Linux AES-256-GCM `secrets.bin`), pembersihan memori sensitif (`zeroize`), penyimpanan JSON atomik, proteksi path traversal/symlink, serta batas ukuran ekstraksi arsip. Detail lengkap ada di [SECURITY.md](SECURITY.md).

---

## Version History / Riwayat Versi

Current version: **`v1.2.1-beta`** (see [CHANGELOG.md](CHANGELOG.md)). The `src-tauri/tauri.conf.json` bundle version is `1.2.1` (without the `-beta` suffix); the workspace version in `Cargo.toml` / `package.json` is `1.2.1-beta`. The internal `@mc/shared` package stays at `0.0.0` — it is not the app version.

### `v1.2.1-beta` — Latest
- **EN:** Custom Tauri GUI installer and uninstaller, plus critical security, launch, modpack, and UI bug fixes.
- **ID:** Installer dan uninstaller GUI Tauri kustom, plus perbaikan bug kritis keamanan, launch, modpack, dan UI.

### `v1.2.0-beta`
- **EN:** Deep debug fixes across launcher crates, IPC, and UI; redundant code comment cleanup.
- **ID:** Perbaikan debug mendalam di seluruh launcher crate, IPC, dan UI; pembersihan komentar kode yang redundan.

### `v1.0.0-beta` — Initial Beta Release
- **EN:** First beta release of MSE Launcher with Vanilla & mod loader management (Fabric, Quilt, NeoForge, Forge), Modrinth & CurseForge browser, Microsoft/offline accounts, 3D skin viewer, isolated instances, Java & RAM manager, server list manager, screenshot gallery, Discord Rich Presence, in-app SHA-256 verified auto-updater (**Portable** vs **Installer**), and compile-time CurseForge proxy support.
- **ID:** Rilis beta pertama MSE Launcher dengan dukungan Vanilla & mod loader (Fabric, Quilt, NeoForge, Forge), browser Modrinth & CurseForge, akun Microsoft/offline, 3D skin viewer, instance terisolasi, pengelola Java & RAM, pengelola daftar server, galeri screenshot, Discord Rich Presence, auto-updater bawaan terverifikasi SHA-256 (**Portable** vs **Installer**), serta dukungan proxy CurseForge saat compile time.

---

## Integrations Configuration

### CurseForge API (Proxy Mode or BYOK)
MSE Launcher does **not** store a hardcoded CurseForge API key in source code.
- **Official Release Binaries (`v1.2.1-beta`):** Ship with a compile-time default proxy endpoint (`MCLAUNCHER_DEFAULT_CF_PROXY`) so CurseForge browsing works out of the box without exposing private URLs in the repository.
- **Source Builds / Custom Configuration:**
  - **Proxy Mode:** Set `CURSEFORGE_PROXY_URL` (or compile-time `MCLAUNCHER_DEFAULT_CF_PROXY`) or set `curseforgeProxyUrl` in `%APPDATA%\MCLauncher\integrations.json` (`~/.local/share/mclauncher/integrations.json` on Linux) pointing to an HTTPS reverse proxy (for example, a Cloudflare Worker that injects `x-api-key` server-side).
  - **Direct API Key (BYOK):** Set `CURSEFORGE_API_KEY` (environment variable) or store `curseforgeApiKey` in `integrations.json` / Settings.

### Microsoft OAuth Client ID
You can supply your own Azure App Registration Client ID via `MS_CLIENT_ID` (environment variable) or `msClientId` in `integrations.json` / Settings.

---

## Network Services & Privacy

The launcher does **not** run analytics or telemetry servers. Outgoing connections go only to:
- **Microsoft / Xbox / Minecraft Services** (`login.live.com`, `user.auth.xboxlive.com`, `xsts.auth.xboxlive.com`, `api.minecraftservices.com`, `textures.minecraft.net`): account login, entitlement check, profile, and skin/cape URLs.
- **Mojang** (`piston-meta.mojang.com`, `piston-data.mojang.com`, `libraries.minecraft.net`, `resources.download.minecraft.net`): game manifests, libraries, assets, and managed Java runtimes.
- **Mod Loaders** (`meta.fabricmc.net`, `maven.fabricmc.net`, `meta.quiltmc.org`, `maven.quiltmc.org`, `meta.neoforged.net`, `maven.minecraftforge.net`, `files.minecraftforge.net`): loader versions and installer artifacts.
- **Mod Platforms** (`api.modrinth.com`, `cdn.modrinth.com`, `api.curseforge.com` or your configured CurseForge proxy, `edge.forgecdn.net`, `mediafilez.forgecdn.net`): searching and downloading mods/modpacks.
- **GitHub Releases** (`github.com`, `api.github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com`): only when checking for or downloading launcher updates.
- **Crafatar** (`crafatar.com`): fallback 3D skin texture lookup by Microsoft account UUID when an account has no cached/uploaded local skin texture URL.
- **Discord Local IPC** (optional): connects only to the local Discord desktop client socket when Rich Presence is enabled.

---

## Architecture

```text
.
├── crates/
│   ├── launcher-archive     # ZIP/mrpack extraction with Zip Slip & size guards
│   ├── launcher-auth        # Microsoft OAuth & offline account manager
│   ├── launcher-content     # Modrinth & CurseForge client, modpack installer
│   ├── launcher-discord     # Local Discord Rich Presence IPC service
│   ├── launcher-download    # Parallel HTTPS downloader with SHA-1/SHA-256 verification
│   ├── launcher-installer   # Custom Tauri GUI installer and uninstaller (Windows)
│   ├── launcher-java        # Mojang JRE resolver & local Java validation
│   ├── launcher-launch      # JVM argument builder & game process lifecycle
│   ├── launcher-loader      # Fabric, Quilt, Forge, NeoForge installer resolver
│   ├── launcher-net         # HTTPS client with retry & downgrade protection
│   ├── launcher-platform    # OS paths, DPAPI/AES-256-GCM secrets, log scrubbing
│   ├── launcher-security    # Path confinement, symlink/junction & filename guards
│   ├── launcher-servers     # Minecraft server ping & servers.dat NBT CRUD
│   ├── launcher-storage     # Atomic JSON persistence & instance store
│   ├── launcher-types       # Shared domain models & error payloads
│   └── launcher-version     # Mojang version manifests & asset downloads
├── src-tauri/               # Tauri v2 desktop shell, IPC commands & auto-updater
├── apps/ui/                 # React 19 + TypeScript + Vite frontend
└── packages/shared/         # Shared TypeScript IPC types & Zod schemas
```

---

## Development

### Prerequisites
- **Node.js** >= 24 (Node 26 supported; CI currently uses Node 24)
- **pnpm** 12.16.1 (see `packageManager` in [package.json](package.json))
- **Rust & Cargo** (`stable`, see [rust-toolchain.toml](rust-toolchain.toml)) — the toolchain is not pinned to a specific Rust version, so builds are not fully reproducible across time
- **Windows:** Microsoft C++ Build Tools (Windows SDK)
- **Linux:** WebKitGTK 4.1 & Tauri v2 system dependencies (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`, `libasound2-dev`)

### Commands

```powershell
# Install workspace dependencies
pnpm install

# Run desktop app in development mode
pnpm dev

# Typecheck TypeScript packages
pnpm typecheck

# Build frontend bundle
pnpm build:ui

# Check formatting, lints, and run Rust tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Build production Tauri installers
pnpm build
```

---

## License

Licensed under the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)**. See [LICENSE](LICENSE), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), and [SECURITY.md](SECURITY.md).
