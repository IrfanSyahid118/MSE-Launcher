import { create } from 'zustand'
import type { LauncherErrorData, LogLine, ProgressEvent } from '@mc/shared'

export type ToastTone = 'info' | 'error'

export interface ToastItem {
  id: number
  message: string
  tone: ToastTone
}

export type SkinModel = 'steve' | 'alex'
export type SkinType = 'steve' | 'alex' | 'microsoft' | 'custom'

/** Satu potongan jejak navigasi di bilah judul. */
export interface BreadcrumbSegment {
  label: string
  /** Rute tujuan; segmen terakhir tidak punya tujuan karena sedang dibuka. */
  to?: string
  /** Aksi kembali untuk halaman yang tidak punya rute sendiri. */
  onClick?: () => void
}

/** Satu pekerjaan instalasi/unduhan yang tampil di dock bawah. */
export interface InstallJob {
  taskId: string
  title: string
  progress: ProgressEvent | null
  logs?: string[]
}

const MAX_PROGRESS_LOG_LINES = 40
const EARLY_EVENT_TTL_MS = 60_000
const MAX_EARLY_EVENTS = 32

export interface BufferedTaskEvent {
  progress?: ProgressEvent
  logs: string[]
  done?: { result: unknown }
  error?: LauncherErrorData
  updatedAt: number
}

const earlyTaskBuffer = new Map<string, BufferedTaskEvent>()

function pruneEarlyBuffer(now: number): void {
  for (const [id, entry] of earlyTaskBuffer) {
    if (now - entry.updatedAt > EARLY_EVENT_TTL_MS) {
      earlyTaskBuffer.delete(id)
    }
  }
  while (earlyTaskBuffer.size >= MAX_EARLY_EVENTS) {
    const oldestKey = earlyTaskBuffer.keys().next().value
    if (!oldestKey) break
    earlyTaskBuffer.delete(oldestKey)
  }
}

export function bufferTaskProgress(taskId: string, progress: ProgressEvent): void {
  const now = Date.now()
  pruneEarlyBuffer(now)
  const existing = earlyTaskBuffer.get(taskId)
  const logs = appendProgressLog(existing?.logs, progress.detail ?? progress.stage)
  earlyTaskBuffer.set(taskId, {
    ...existing,
    progress,
    logs,
    updatedAt: now,
  })
}

export function bufferTaskDone(taskId: string, result: unknown): void {
  const now = Date.now()
  pruneEarlyBuffer(now)
  const existing = earlyTaskBuffer.get(taskId)
  earlyTaskBuffer.set(taskId, {
    logs: existing?.logs ?? [],
    ...existing,
    done: { result },
    updatedAt: now,
  })
}

export function bufferTaskError(taskId: string, error: LauncherErrorData): void {
  const now = Date.now()
  pruneEarlyBuffer(now)
  const existing = earlyTaskBuffer.get(taskId)
  earlyTaskBuffer.set(taskId, {
    logs: existing?.logs ?? [],
    ...existing,
    error,
    updatedAt: now,
  })
}

export function consumeBufferedTaskEvent(taskId: string): BufferedTaskEvent | undefined {
  const entry = earlyTaskBuffer.get(taskId)
  if (!entry) return undefined
  earlyTaskBuffer.delete(taskId)
  return entry
}

function appendProgressLog(prev: string[] | undefined, line: string | undefined): string[] {
  const base = prev ?? []
  const trimmed = line?.trim()
  if (!trimmed) return base
  if (base[base.length - 1] === trimmed) return base
  const next = [...base, trimmed]
  return next.length > MAX_PROGRESS_LOG_LINES
    ? next.slice(next.length - MAX_PROGRESS_LOG_LINES)
    : next
}

export interface LaunchErrorSummary {
  message: string
  time: string
}

interface AppState {
  /** Instance yang dipilih di Home; Play selalu memakai instance ini. */
  selectedInstanceId: string | null
  /** Model skin yang dipilih ('steve' atau 'alex'). */
  selectedSkin: SkinModel
  /** Tipe sumber skin (steve, alex, microsoft, atau custom); null = ikuti akun aktif. */
  selectedSkinType: SkinType | null
  /** Pilihan tipe sumber skin per akun (`account.id -> SkinType`). */
  accountSkinTypes: Record<string, SkinType>
  /** Skin kustom per akun (`account.id -> data URL`) sebelum sinkronisasi server selesai. */
  customSkinsByAccount: Record<string, string>
  /**
   * Model lengan karakter: `false` = klasik (4 px), `true` = slim (3 px).
   * Terpisah dari tekstur skin, karena tekstur yang sama bisa dipakai
   * kedua model.
   */
  skinSlim: boolean
  /** Data URL tekstur skin custom (.png) yang diunggah pengguna (saat tanpa akun). */
  customSkinUrl: string | null
  /** Instance yang proses game-nya sedang disiapkan/berjalan. */
  launchInstanceId: string | null
  /** Instance terakhir yang pernah diluncurkan (tetap tersimpan saat game keluar). */
  lastLaunchInstanceId: string | null
  launchTaskId: string | null
  launchProgress: ProgressEvent | null
  launchLogs: string[]
  cleanupTaskId: string | null
  isGameRunning: boolean
  consoleLines: LogLine[]
  launcherLogs: LogLine[]
  lastLaunchError: LaunchErrorSummary | null
  toasts: ToastItem[]
  /** Pekerjaan instalasi aktif, ditampilkan sebagai dock di bawah layar. */
  installJobs: InstallJob[]
  /** Jejak halaman mendalam (mis. Discover modpacks > proyek > versi). */
  breadcrumb: BreadcrumbSegment[] | null
  setBreadcrumb: (trail: BreadcrumbSegment[] | null) => void
  upsertInstallJob: (job: InstallJob) => void
  removeInstallJob: (taskId: string) => void
  setSelectedInstance: (instanceId: string | null) => void
  setSelectedSkin: (skin: SkinModel) => void
  setSelectedSkinType: (type: SkinType | null) => void
  setAccountSkinType: (accountId: string, type: SkinType | null) => void
  setAccountCustomSkin: (accountId: string, url: string | null) => void
  setSkinSlim: (slim: boolean) => void
  setCustomSkinUrl: (url: string | null) => void
  setLaunchInstanceId: (instanceId: string | null) => void
  setLastLaunchInstanceId: (instanceId: string | null) => void
  setLaunchTaskId: (taskId: string | null) => void
  setLaunchProgress: (progress: ProgressEvent | null) => void
  restoreLaunchLogs: (logs: string[]) => void
  setCleanupTaskId: (taskId: string | null) => void
  setIsGameRunning: (running: boolean) => void
  pushConsoleLines: (lines: LogLine[]) => void
  clearConsole: () => void
  pushLauncherLogs: (lines: LogLine[]) => void
  setLauncherLogs: (lines: LogLine[]) => void
  clearLauncherLogs: () => void
  setLastLaunchError: (err: LaunchErrorSummary | null) => void
  pushToast: (message: string, tone?: ToastTone) => void
  dismissToast: (id: number) => void
}

let nextToastId = 1

const initialSkin: SkinModel =
  typeof window !== 'undefined' && window.localStorage?.getItem('mc_selected_skin') === 'alex'
    ? 'alex'
    : 'steve'

const VALID_SKIN_TYPES = new Set<SkinType>(['steve', 'alex', 'microsoft', 'custom'])

const initialSkinType: SkinType | null = (() => {
  if (typeof window === 'undefined') return null
  const raw = window.localStorage?.getItem('mc_selected_skin_type')
  return raw && VALID_SKIN_TYPES.has(raw as SkinType) ? (raw as SkinType) : null
})()

function loadAccountSkinTypes(): Record<string, SkinType> {
  if (typeof window === 'undefined') return {}
  try {
    const raw = window.localStorage?.getItem('mc_account_skin_types')
    if (!raw) return {}
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    const out: Record<string, SkinType> = {}
    for (const [k, v] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof v === 'string' && VALID_SKIN_TYPES.has(v as SkinType)) {
        out[k] = v as SkinType
      }
    }
    return out
  } catch {
    return {}
  }
}

const initialAccountSkinTypes: Record<string, SkinType> = loadAccountSkinTypes()

const initialCustomSkin: string | null =
  typeof window !== 'undefined' ? (window.localStorage?.getItem('mc_custom_skin') ?? null) : null

const initialSkinSlim: boolean =
  typeof window !== 'undefined'
    ? window.localStorage?.getItem('mc_skin_slim') === 'true'
    : false

export const useAppStore = create<AppState>((set) => ({
  selectedInstanceId: null,
  selectedSkin: initialSkin,
  selectedSkinType: initialSkinType,
  accountSkinTypes: initialAccountSkinTypes,
  customSkinsByAccount: {},
  skinSlim: initialSkinSlim,
  customSkinUrl: initialCustomSkin,
  launchInstanceId: null,
  lastLaunchInstanceId: null,
  launchTaskId: null,
  launchProgress: null,
  launchLogs: [],
  cleanupTaskId: null,
  isGameRunning: false,
  consoleLines: [],
  launcherLogs: [],
  lastLaunchError: null,
  toasts: [],
  breadcrumb: null,
  installJobs: [],
  setBreadcrumb: (trail) => {
    set({ breadcrumb: trail })
  },
  upsertInstallJob: (job) => {
    set((state) => {
      const logEntry = job.progress?.detail ?? job.progress?.stage
      const existing = state.installJobs.findIndex((entry) => entry.taskId === job.taskId)
      if (existing === -1) {
        const logs = job.logs ?? appendProgressLog([], logEntry)
        return { installJobs: [...state.installJobs, { ...job, logs }] }
      }
      const next = [...state.installJobs]
      const prev = next[existing]
      const logs = job.logs ?? appendProgressLog(prev?.logs, logEntry)
      next[existing] = { ...job, logs }
      return { installJobs: next }
    })
  },
  removeInstallJob: (taskId) => {
    set((state) => ({
      installJobs: state.installJobs.filter((entry) => entry.taskId !== taskId),
    }))
  },
  setSelectedInstance: (instanceId) => {
    set({ selectedInstanceId: instanceId })
  },
  setSelectedSkin: (skin) => {
    if (typeof window !== 'undefined') {
      try {
        window.localStorage.setItem('mc_selected_skin', skin)
      } catch {
      }
    }
    set({ selectedSkin: skin })
  },
  setSelectedSkinType: (type) => {
    if (typeof window !== 'undefined') {
      try {
        if (type) {
          window.localStorage.setItem('mc_selected_skin_type', type)
        } else {
          window.localStorage.removeItem('mc_selected_skin_type')
        }
      } catch {
      }
    }
    set({ selectedSkinType: type })
  },
  setAccountSkinType: (accountId, type) => {
    set((state) => {
      const next = { ...state.accountSkinTypes }
      if (type) {
        next[accountId] = type
      } else {
        delete next[accountId]
      }
      if (typeof window !== 'undefined') {
        try {
          window.localStorage.setItem('mc_account_skin_types', JSON.stringify(next))
        } catch {
        }
      }
      return { accountSkinTypes: next }
    })
  },
  setAccountCustomSkin: (accountId, url) => {
    set((state) => {
      const next = { ...state.customSkinsByAccount }
      if (url) {
        next[accountId] = url
      } else {
        delete next[accountId]
      }
      return { customSkinsByAccount: next }
    })
  },
  setSkinSlim: (slim) => {
    if (typeof window !== 'undefined') {
      try {
        window.localStorage.setItem('mc_skin_slim', slim ? 'true' : 'false')
      } catch {
      }
    }
    set({ skinSlim: slim })
  },
  setCustomSkinUrl: (url) => {
    if (typeof window !== 'undefined') {
      try {
        if (url) {
          window.localStorage.setItem('mc_custom_skin', url)
        } else {
          window.localStorage.removeItem('mc_custom_skin')
        }
      } catch {
      }
    }
    set({ customSkinUrl: url })
  },
  setLaunchInstanceId: (instanceId) => set({ launchInstanceId: instanceId }),
  setLastLaunchInstanceId: (instanceId) => set({ lastLaunchInstanceId: instanceId }),
  setLaunchTaskId: (taskId) =>
    set((state) => ({
      launchTaskId: taskId,
      lastLaunchError: taskId ? null : state.lastLaunchError,
    })),
  setLaunchProgress: (progress) =>
    set((state) => ({
      launchProgress: progress,
      launchLogs:
        progress === null
          ? []
          : appendProgressLog(state.launchLogs, progress.detail ?? progress.stage),
    })),
  restoreLaunchLogs: (logs) =>
    set((state) => ({
      launchLogs: logs.length > 0 ? logs.slice(-MAX_PROGRESS_LOG_LINES) : state.launchLogs,
    })),
  setCleanupTaskId: (taskId) => set({ cleanupTaskId: taskId }),
  setIsGameRunning: (isGameRunning) => set({ isGameRunning }),
  pushConsoleLines: (lines) => {
    if (lines.length === 0) return
    set((state) => {
      const keep = 1_500 - Math.min(lines.length, 1_500)
      const next = state.consoleLines.length + lines.length <= 1_500
        ? [...state.consoleLines, ...lines]
        : [...(keep > 0 ? state.consoleLines.slice(-keep) : []), ...lines.slice(-1_500)]
      return { consoleLines: next }
    })
  },
  clearConsole: () => set({ consoleLines: [] }),
  pushLauncherLogs: (lines) => {
    if (lines.length === 0) return
    set((state) => {
      const keep = 1_500 - Math.min(lines.length, 1_500)
      const next = state.launcherLogs.length + lines.length <= 1_500
        ? [...state.launcherLogs, ...lines]
        : [...(keep > 0 ? state.launcherLogs.slice(-keep) : []), ...lines.slice(-1_500)]
      return { launcherLogs: next }
    })
  },
  setLauncherLogs: (lines) => set({ launcherLogs: lines.slice(-1_500) }),
  clearLauncherLogs: () => set({ launcherLogs: [] }),
  setLastLaunchError: (err) => set({ lastLaunchError: err }),
  pushToast: (message, tone = 'info') => {
    set((state) => ({
      toasts: [...state.toasts, { id: nextToastId++, message, tone }],
    }))
  },
  dismissToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) }))
  },
}))
