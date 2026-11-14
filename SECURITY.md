# Security Policy

## Supported Versions

| Version | Supported |
| --- | --- |
| 1.0.x (Beta) | Yes |
| < 1.0.0 | No |

## Reporting a Vulnerability

Please report security vulnerabilities privately using **GitHub Private Vulnerability Reporting** (**Security -> Report a vulnerability**) on this repository. Do **not** open public GitHub issues, pull requests, or discussions for undisclosed vulnerabilities.

Target timelines:
- **Initial acknowledgment:** within 72 hours
- **Triage and remediation plan:** within 7 days

Please include steps to reproduce, affected platforms (Windows x64 / Linux x64), and any proof-of-concept details needed to verify the fix.

## Threat Model & Local Security Boundaries

### Credential Storage (`secrets.bin`)
- **Windows:** OAuth refresh and access tokens are encrypted with Windows DPAPI (`CryptProtectData`), scoped to the active Windows user account. Any code or malware running under the same Windows user session can invoke `CryptUnprotectData`.
- **Linux:** `secrets.bin` is encrypted with AES-256-GCM (`ring`), using a randomly generated 256-bit key stored in `secrets.bin.key` alongside `secrets.bin` with restrictive `0600` (`rw-------`) file permissions inside `0700` data directories. This isolates secrets from other non-root local users on multi-user systems, **not** from processes running under the same Linux UID (D-Bus Secret Service / `libsecret` integration is not yet implemented).
- **Memory & Log Hygiene:** Token buffers are zeroized on drop (`zeroize`). Logs and offline diagnostic bundles (`MCSE1:...`) scrub OAuth tokens, authorization headers, `x-api-key` values, and local OS user paths.

### Trust Boundaries & Game Execution
- **Mods, Modpacks, and Mod Loaders:** Minecraft `.jar` mods, resource packs, and Forge/NeoForge/Fabric/Quilt installers execute arbitrary JVM code with the current OS user's permissions once launched. Only install content from trusted sources.
- **Custom JVM Arguments:** `custom_jvm_args` in global and instance settings are checked against a blocklist (`-javaagent`, `-agentlib`, `-agentpath`, `OnError`, `OnOutOfMemoryError`, remote JMX) and a safe-flag allowlist before spawning the JVM.
- **Path Confinement & Archives:** Archive extraction, instance management, world saves, asset downloads, and log access enforce canonical boundary checks, disallow path traversal, reserved Windows device names, and symlinks/NTFS junctions escaping the data root (`launcher-security`).
- **HTTPS Transport:** External network requests require HTTPS in release builds and reject HTTPS-to-HTTP downgrade redirects.
