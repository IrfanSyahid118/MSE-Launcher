export type ErrorCode =
  | 'NETWORK'
  | 'RATE_LIMITED'
  | 'NOT_FOUND'
  | 'ALREADY_EXISTS'
  | 'HASH_MISMATCH'
  | 'DISK_FULL'
  | 'FILE_SYSTEM'
  | 'JAVA_MISSING'
  | 'AUTH_FAILED'
  | 'AUTH_EXPIRED'
  | 'PROVIDER_NOT_CONFIGURED'
  | 'DISTRIBUTION_BLOCKED'
  | 'INVALID_INPUT'
  | 'LAUNCH_FAILED'
  | 'CANCELLED'
  | 'INTERNAL'

export interface LauncherErrorData {
  code: ErrorCode
  /** Pesan siap tampil, bukan stack trace. */
  message: string
  retryable: boolean
  details?: unknown
}

const DEFAULT_MESSAGES: Record<ErrorCode, string> = {
  NETWORK: 'Koneksi ke server gagal. Periksa jaringan lalu coba lagi.',
  RATE_LIMITED: 'Terlalu banyak permintaan ke server. Coba lagi beberapa saat.',
  NOT_FOUND: 'Data yang diminta tidak ditemukan.',
  ALREADY_EXISTS: 'Data dengan identitas tersebut sudah ada.',
  HASH_MISMATCH: 'File yang diunduh rusak. Coba unduh ulang.',
  DISK_FULL: 'Ruang penyimpanan tidak cukup.',
  FILE_SYSTEM: 'Operasi berkas gagal. Periksa izin dan direktori.',
  JAVA_MISSING: 'Java yang dibutuhkan tidak ditemukan.',
  AUTH_FAILED: 'Autentikasi gagal. Coba masuk kembali.',
  AUTH_EXPIRED: 'Sesi akun sudah kedaluwarsa. Masuk kembali.',
  PROVIDER_NOT_CONFIGURED: 'Layanan ini belum dikonfigurasi.',
  DISTRIBUTION_BLOCKED: 'Pembuat konten melarang unduhan pihak ketiga.',
  INVALID_INPUT: 'Data yang dikirim tidak valid.',
  LAUNCH_FAILED: 'Gagal meluncurkan proses game.',
  CANCELLED: 'Operasi dibatalkan.',
  INTERNAL: 'Terjadi kesalahan internal. Lihat log launcher.',
}

export function defaultMessageFor(code: ErrorCode): string {
  return DEFAULT_MESSAGES[code]
}

export interface LauncherErrorOptions {
  retryable?: boolean
  details?: unknown
  cause?: unknown
}

export class LauncherError extends Error {
  readonly code: ErrorCode
  readonly retryable: boolean
  readonly details?: unknown

  constructor(code: ErrorCode, message?: string, options: LauncherErrorOptions = {}) {
    super(message ?? defaultMessageFor(code), options.cause === undefined ? undefined : { cause: options.cause })
    this.name = 'LauncherError'
    this.code = code
    this.retryable = options.retryable ?? (code === 'NETWORK' || code === 'RATE_LIMITED')
    if (options.details !== undefined) {
      this.details = options.details
    }
  }

  toData(): LauncherErrorData {
    const data: LauncherErrorData = {
      code: this.code,
      message: this.message,
      retryable: this.retryable,
    }
    if (this.details !== undefined) {
      data.details = this.details
    }
    return data
  }
}

export function isLauncherError(value: unknown): value is LauncherError {
  return value instanceof LauncherError
}

/** Mengubah error apa pun menjadi data yang aman melewati IPC. */
export function toLauncherErrorData(value: unknown): LauncherErrorData {
  if (value instanceof LauncherError) {
    return value.toData()
  }
  return { code: 'INTERNAL', message: defaultMessageFor('INTERNAL'), retryable: false }
}
