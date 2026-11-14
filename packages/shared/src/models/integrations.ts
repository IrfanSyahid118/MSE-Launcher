/** Urutan sumber kunci sesuai §10.3: yang pertama ditemukan dipakai. */
export type IntegrationSource = 'env' | 'config' | 'settings' | 'proxy' | 'builtin'

/**
 * Status satu integrasi yang butuh kunci. `source` menunjukkan dari mana kunci
 * yang sedang dipakai berasal, supaya pengguna tahu mengapa kolom di Settings
 * tidak berpengaruh.
 */
export interface IntegrationStatus {
  configured: boolean
  source: IntegrationSource | null
  /** true bila kunci tersimpan lewat Settings, walau sumber lain menimpanya. */
  stored: boolean
}

export interface IntegrationsStatus {
  curseforge: IntegrationStatus
  azure?: IntegrationStatus | undefined
}
