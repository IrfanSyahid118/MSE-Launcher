export type AccountType = 'microsoft' | 'offline'

export interface AccountCape {
  id: string
  state: 'ACTIVE' | 'INACTIVE'
  url: string
  alias?: string | undefined
}

export interface Account {
  id: string
  username: string
  uuid: string
  type: AccountType
  createdAt: string
  /** ISO timestamp for Microsoft sessions, null for offline accounts. */
  expiresAt: string | null
  /** Secure tokens for Microsoft authentication, stored in secrets.bin. */
  tokens?: {
    accessToken: string
    refreshToken: string
  } | undefined
  xuid?: string | undefined
  skinUrl?: string | undefined
  skinVariant?: 'classic' | 'slim' | undefined
  capeUrl?: string | undefined
  activeCapeId?: string | undefined
  capes?: AccountCape[] | undefined
}
export interface AccountsFile {
  schemaVersion: 1
  accounts: Account[]
}

export const ACCOUNTS_SCHEMA_VERSION = 1

export function emptyAccountsFile(): AccountsFile {
  return { schemaVersion: ACCOUNTS_SCHEMA_VERSION, accounts: [] }
}
