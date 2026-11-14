/** Ringkasan hasil pembersihan data unduhan bersama setelah instance dihapus. */
export interface CleanupSummary {
  removedFiles: number
  freedBytes: number
  warnings: string[]
}
