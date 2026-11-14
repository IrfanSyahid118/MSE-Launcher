import { create } from 'zustand'
import type { LogLine, ProgressEvent } from '@mc/shared'

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
  /** Tipe sumber skin (steve, alex, microsoft, atau custom). */
  selectedSkinType: SkinType
  /**
   * Model lengan karakter: `false` = klasik (4 px), `true` = slim (3 px).
   * Terpisah dari tekstur skin, karena tekstur yang sama bisa dipakai
   * kedua model.
   */
  skinSlim: boolean
  /** Data URL tekstur skin custom (.png) yang diunggah pengguna. */
  customSkinUrl: string | null
  /** Instance yang proses game-nya sedang disiapkan/berjalan. */
  launchInstanceId: string | null
  /** Instance terakhir yang pernah diluncurkan (tetap tersimpan saat game keluar). */
  lastLaunchInstanceId: string | null
  launchTaskId: string | null
  launchProgress: ProgressEvent | null
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
  setSelectedSkinType: (type: SkinType) => void
  setSkinSlim: (slim: boolean) => void
  setCustomSkinUrl: (url: string | null) => void
  setLaunchInstanceId: (instanceId: string | null) => void
  setLastLaunchInstanceId: (instanceId: string | null) => void
  setLaunchTaskId: (taskId: string | null) => void
  setLaunchProgress: (progress: ProgressEvent | null) => void
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

const initialSkinType: SkinType =
  typeof window !== 'undefined'
    ? ((window.localStorage?.getItem('mc_selected_skin_type') as SkinType) ?? 'steve')
    : 'steve'

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
  skinSlim: initialSkinSlim,
  customSkinUrl: initialCustomSkin,
  launchInstanceId: null,
  lastLaunchInstanceId: null,
  launchTaskId: null,
  launchProgress: null,
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
      const existing = state.installJobs.findIndex((entry) => entry.taskId === job.taskId)
      if (existing === -1) {
        return { installJobs: [...state.installJobs, job] }
      }
      const next = [...state.installJobs]
      next[existing] = job
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
        // Abaikan jika localStorage tidak tersedia di environment tertentu
      }
    }
    set({ selectedSkin: skin })
  },
  setSelectedSkinType: (type) => {
    if (typeof window !== 'undefined') {
      try {
        window.localStorage.setItem('mc_selected_skin_type', type)
      } catch {
        // Abaikan jika localStorage tidak tersedia di environment tertentu
      }
    }
    set({ selectedSkinType: type })
  },
  setSkinSlim: (slim) => {
    if (typeof window !== 'undefined') {
      try {
        window.localStorage.setItem('mc_skin_slim', slim ? 'true' : 'false')
      } catch {
        // Abaikan jika localStorage tidak tersedia di environment tertentu
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
        // Abaikan jika localStorage tidak tersedia
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
  setLaunchProgress: (progress) => set({ launchProgress: progress }),
  setCleanupTaskId: (taskId) => set({ cleanupTaskId: taskId }),
  setIsGameRunning: (isGameRunning) => set({ isGameRunning }),
  pushConsoleLines: (lines) => {
    if (lines.length === 0) return
    set((state) => {
      const next = state.consoleLines.length + lines.length <= 1_500
        ? [...state.consoleLines, ...lines]
        : [...state.consoleLines.slice(-(1_500 - Math.min(lines.length, 1_500))), ...lines.slice(-1_500)]
      return { consoleLines: next }
    })
  },
  clearConsole: () => set({ consoleLines: [] }),
  pushLauncherLogs: (lines) => {
    if (lines.length === 0) return
    set((state) => {
      const next = state.launcherLogs.length + lines.length <= 1_500
        ? [...state.launcherLogs, ...lines]
        : [...state.launcherLogs.slice(-(1_500 - Math.min(lines.length, 1_500))), ...lines.slice(-1_500)]
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
