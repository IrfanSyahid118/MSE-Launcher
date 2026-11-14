# Third-Party Notices

MSE Launcher incorporates third-party open-source packages across its Rust workspace and web UI. Full license texts for each dependency are preserved in their upstream distributions.

## Fonts
- **`@fontsource/inter`** — SIL Open Font License 1.1 (`OFL-1.1`), Copyright The Inter Project Authors.

## Cryptography & Security Primitives
- **`ring`** — `Apache-2.0 AND ISC` (includes BoringSSL/OpenSSL-derived cryptographic routines subject to their respective permissive notices).

## Weak Copyleft (`MPL-2.0`) Components
Unmodified upstream components licensed under the Mozilla Public License 2.0 (`MPL-2.0`) include:
- Rust crates: `option-ext`, `infer`, `urlpattern`, `tauri-utils`, `tauri-runtime-wry`
- JavaScript packages: `lightningcss`, `lightningcss-win32-x64-msvc` (and platform binaries)

Source code for unmodified `MPL-2.0` packages is available from crates.io and npmjs.com under the terms of the Mozilla Public License 2.0.

## Other Direct & Transitive Dependencies
Remaining Rust (`Cargo.lock`) and JavaScript (`pnpm-lock.yaml`) dependencies—including `tauri`, `tokio`, `reqwest`, `zip`, `react`, `three`, `lucide-react`, `radix-ui`, and `zod`—are licensed under `MIT`, `Apache-2.0`, `ISC`, `BSD-3-Clause`, `CC-BY-4.0` (`caniuse-lite`), or `0BSD` (`tslib`).
