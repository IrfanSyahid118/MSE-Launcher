export type MinecraftVersionType = 'release' | 'snapshot' | 'old_beta' | 'old_alpha'

/** Ringkasan versi untuk pemilih versi di UI. */
export interface MinecraftVersionInfo {
  id: string
  type: MinecraftVersionType
  releaseTime: string
  /** true jika `shared/versions/<id>/<id>.json` sudah ada di disk. */
  installed: boolean
}

/** Satu versi loader yang bisa dipilih untuk sebuah versi Minecraft. */
export interface LoaderVersionInfo {
  version: string
  stable: boolean
}

/**
 * Daftar versi Minecraft yang didukung tiap loader. `null` berarti daftar
 * tidak bisa diambil (misalnya sedang offline), bukan berarti tidak ada versi.
 */
export interface LoaderGameSupport {
  fabric: string[] | null
  quilt: string[] | null
  forge: string[] | null
  neoforge: string[] | null
}

/** Bandingkan dua versi (Minecraft, mod, atau modpack) secara numerik per segmen, terbaru lebih dulu. */
export function compareMinecraftVersions(left: string, right: string): number {
  const leftParts = left.split(/[.+-]/)
  const rightParts = right.split(/[.+-]/)
  const length = Math.max(leftParts.length, rightParts.length)
  for (let i = 0; i < length; i++) {
    const l = Number.parseInt(leftParts[i] ?? '0', 10)
    const r = Number.parseInt(rightParts[i] ?? '0', 10)
    const lNum = Number.isNaN(l) ? 0 : l
    const rNum = Number.isNaN(r) ? 0 : r
    if (lNum !== rNum) {
      return rNum - lNum
    }
  }
  return right.localeCompare(left)
}

/** Apakah versi Minecraft ini mendukung Datapack (diperkenalkan di 1.13). */
export function supportsDatapacks(minecraftVersion: string): boolean {
  if (!minecraftVersion) return false
  return compareMinecraftVersions(minecraftVersion, '1.13') <= 0
}

/**
 * Cek apakah sebuah versi konten cocok dengan versi Minecraft target,
 * termasuk penanganan kasus upload liar (author mencentang 'Select All' 900 versi).
 */
export function isVersionCompatibleWithMinecraft(
  gameVersions: string[],
  targetMinecraftVersion: string,
  versionNumber?: string,
  versionName?: string,
): boolean {
  if (!targetMinecraftVersion) return true
  if (gameVersions.length > 0 && !gameVersions.includes(targetMinecraftVersion)) {
    return false
  }

  // Heuristic for mass-tagged ("Select All") uploads that claim dozens of versions.
  if (
    gameVersions.length > 25 ||
    (gameVersions.length > 10 &&
      gameVersions.some((v) => v.startsWith('rd-') || v.startsWith('c0.')))
  ) {
    const textToScan = `${versionNumber ?? ''} ${versionName ?? ''}`
    const matches = textToScan.match(/\b(1\.\d+(?:\.\d+)?)\b/g)
    if (matches && matches.length > 0) {
      const targetMajorMinor = targetMinecraftVersion.split('.').slice(0, 2).join('.')
      const hasMatchingMc = matches.some((m) => {
        const mMajorMinor = m.split('.').slice(0, 2).join('.')
        return m === targetMinecraftVersion || mMajorMinor === targetMajorMinor
      })
      if (!hasMatchingMc) {
        return false
      }
    }
  }

  return true
}
