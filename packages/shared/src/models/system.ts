export interface MemoryInfo {
  totalMb: number
  freeMb: number
}

export interface AppInfo {
  appVersion: string
  electronVersion: string
  chromeVersion: string
  platform: string
  arch: string
}

export type InstallMode = 'installer' | 'portable'

export interface UpdateCheckResult {
  currentVersion: string
  latestVersion: string
  updateAvailable: boolean
  installMode: InstallMode
  assetName?: string | undefined
  downloadUrl?: string | undefined
  sizeBytes?: number | undefined
  sha256?: string | undefined
  releaseNotes?: string | undefined
}
