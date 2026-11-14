export interface JavaInstallation {
  path: string
  version: string
  majorVersion: number
  bits: 32 | 64
  vendor: string | null
  managed: boolean
}

export interface DeviceCodeInfo {
  deviceCode: string
  userCode: string
  verificationUri: string
  expiresIn: number
  interval: number
}
