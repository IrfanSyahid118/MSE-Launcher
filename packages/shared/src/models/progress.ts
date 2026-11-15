export interface ProgressEvent {
  stage: string
  completed: number
  total: number
  /** Detail berkas atau aktivitas berjalan untuk baris konsol unduhan. */
  detail?: string | undefined
  /**
   * Indeks tahap dalam seluruh alur kerja (0, 1, 2, ...). UI memakainya agar
   * persen tidak melompat mundur saat berganti tahap unduhan.
   */
  stageIndex?: number | undefined
  /** Jumlah tahap yang diketahui; 0 bila alurnya tidak bertahap. */
  stageCount?: number | undefined
}

export type LogStream = 'stdout' | 'stderr' | 'system'

export interface LogLine {
  stream: LogStream
  line: string
  /** ISO timestamp. */
  time: string
}

export interface DiagnosticReportResult {
  refId: string
  code: string
  filePath: string
  reportText: string
  launcherLines: number
  gameLines: number
}
