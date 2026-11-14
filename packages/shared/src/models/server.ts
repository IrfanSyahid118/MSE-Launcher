export type ServerResourcePackPolicy = 'prompt' | 'enabled' | 'disabled'

export interface InstanceServerEntry {
  name: string
  ip: string
  icon?: string | null | undefined
  acceptTextures?: number | undefined
  resourcePackPolicy?: ServerResourcePackPolicy | undefined
  hideFromHome?: boolean | undefined
}

export interface ServerPingStatus {
  ip: string
  online: boolean
  latencyMs: number | null
  playersOnline: number | null
  playersMax: number | null
  motd: string | null
  versionName: string | null
  favicon: string | null
  checkedAt: string
}
