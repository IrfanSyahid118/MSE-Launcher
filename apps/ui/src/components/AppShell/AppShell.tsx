import { Suspense, lazy, useEffect, useMemo } from 'react'
import type { ReactNode } from 'react'
import type { CleanupSummary, LogLine } from '@mc/shared'
import { useQueryClient } from '@tanstack/react-query'
import { Navigate, Route, Routes, useLocation } from 'react-router'
import { AnimatePresence, motion } from 'motion/react'
import { useI18n } from '../../i18n'
import { invoke } from '../../api/launcher'
import { queryKeys, useActiveAccount, useInstances } from '../../api/queries'
import { useAppStore } from '../../stores/app-store'
import type { BreadcrumbSegment } from '../../stores/app-store'
import { GlassSidebar } from '../GlassSidebar/GlassSidebar'
import { HintProvider } from '../Hint/Hint'
import { InstallDock } from '../InstallDock/InstallDock'
import { TitleBar } from '../TitleBar/TitleBar'
import { Toaster } from '../Toaster/Toaster'
import { Home } from '../../screens/Home'
import styles from './AppShell.module.css'

const CommandPalette = lazy(() =>
  import('../CommandPalette/CommandPalette').then((m) => ({ default: m.CommandPalette })),
)
const Settings = lazy(() =>
  import('../../screens/Settings').then((m) => ({ default: m.Settings })),
)
const Console = lazy(() =>
  import('../../screens/Console').then((m) => ({ default: m.Console })),
)
const Instances = lazy(() =>
  import('../../screens/Instances').then((m) => ({ default: m.Instances })),
)
const InstanceDetail = lazy(() =>
  import('../../screens/InstanceDetail').then((m) => ({ default: m.InstanceDetail })),
)
const Browse = lazy(() =>
  import('../../screens/Browse').then((m) => ({ default: m.Browse })),
)
const Accounts = lazy(() =>
  import('../../screens/Accounts').then((m) => ({ default: m.Accounts })),
)
const SkinSelector = lazy(() =>
  import('../../screens/SkinSelector').then((m) => ({ default: m.SkinSelector })),
)
const Screenshots = lazy(() =>
  import('../../screens/Screenshots').then((m) => ({ default: m.Screenshots })),
)

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`
  }
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`
}

function ShellLayout(): ReactNode {
  const { t } = useI18n()
  const location = useLocation()
  const queryClient = useQueryClient()
  const { account, isLoading } = useActiveAccount()
  const instancesQuery = useInstances()
  const pushConsoleLines = useAppStore((state) => state.pushConsoleLines)
  const pushLauncherLogs = useAppStore((state) => state.pushLauncherLogs)
  const setLauncherLogs = useAppStore((state) => state.setLauncherLogs)
  const setLastLaunchError = useAppStore((state) => state.setLastLaunchError)
  const setLaunchTaskId = useAppStore((state) => state.setLaunchTaskId)
  const setLaunchInstanceId = useAppStore((state) => state.setLaunchInstanceId)
  const setLaunchProgress = useAppStore((state) => state.setLaunchProgress)
  const setCleanupTaskId = useAppStore((state) => state.setCleanupTaskId)
  const setIsGameRunning = useAppStore((state) => state.setIsGameRunning)
  const pushToast = useAppStore((state) => state.pushToast)
  const upsertInstallJob = useAppStore((state) => state.upsertInstallJob)
  const removeInstallJob = useAppStore((state) => state.removeInstallJob)
  const deepTrail = useAppStore((state) => state.breadcrumb)
  const setBreadcrumb = useAppStore((state) => state.setBreadcrumb)

  const trail = useMemo<BreadcrumbSegment[]>(() => {
    const path = location.pathname
    if (path.startsWith('/browse')) {
      return deepTrail ?? [{ label: t('nav.browse') }]
    }
    if (path.startsWith('/instances/')) {
      const id = path.slice('/instances/'.length)
      const instance = (instancesQuery.data ?? []).find((entry) => entry.id === id)
      return [
        { label: t('nav.instances'), to: '/instances' },
        { label: instance?.name ?? t('instances.detail') },
      ]
    }
    if (path.startsWith('/instances')) return [{ label: t('nav.instances') }]
    if (path.startsWith('/settings')) return [{ label: t('nav.settings') }]
    if (path.startsWith('/accounts')) return [{ label: t('nav.accounts') }]
    if (path.startsWith('/skins')) return [{ label: t('nav.skins') }]
    if (path.startsWith('/screenshots')) return [{ label: t('nav.screenshots') }]
    if (path.startsWith('/console')) return [{ label: t('nav.console') }]
    return [{ label: t('nav.home') }]
  }, [deepTrail, instancesQuery.data, location.pathname, t])

  useEffect(() => {
    if (!location.pathname.startsWith('/browse') && deepTrail !== null) {
      setBreadcrumb(null)
    }
  }, [deepTrail, location.pathname, setBreadcrumb])

  useEffect(() => {
    let menuAnnounced = false
    let realmsNoted = false
    const systemLine = (line: string): LogLine => ({
      stream: 'system',
      line,
      time: new Date().toISOString(),
    })

    const offProgress = window.launcher.on('task:progress', ({ taskId, progress }) => {
      if (useAppStore.getState().launchTaskId === taskId) {
        setLaunchProgress(progress)
        return
      }
      const jobs = useAppStore.getState().installJobs
      const job = jobs.find((entry) => entry.taskId === taskId)
      if (job) {
        upsertInstallJob({ taskId, title: job.title, progress })
      }
    })
    const offDone = window.launcher.on('task:done', ({ taskId, result }) => {
      if (useAppStore.getState().cleanupTaskId === taskId) {
        setCleanupTaskId(null)
        const summary = result as CleanupSummary | undefined
        pushToast(
          t('instances.cleanupDone', {
            files: summary?.removedFiles ?? 0,
            size: formatBytes(summary?.freedBytes ?? 0),
          }),
        )
        return
      }
      if (useAppStore.getState().launchTaskId === taskId) {
        setLaunchTaskId(null)
        setLaunchProgress(null)
        setIsGameRunning(true)
        // Refresh instances so updated lastPlayedAt and version appear.
        void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
        return
      }
      const jobs = useAppStore.getState().installJobs
      const job = jobs.find((entry) => entry.taskId === taskId)
      if (job) {
        void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
        pushToast(t('browse.installDone', { title: job.title }))
        removeInstallJob(taskId)
        void result
      }
    })
    const offError = window.launcher.on('task:error', ({ taskId, error }) => {
      void invoke('logs:appendClient', {
        stream: 'stderr',
        line: `Task error (${taskId}): [${error.code}] ${error.message}`,
      }).catch(() => {})
      if (useAppStore.getState().cleanupTaskId === taskId) {
        setCleanupTaskId(null)
        pushToast(error.message, 'error')
        return
      }
      if (useAppStore.getState().launchTaskId === taskId) {
        setLaunchTaskId(null)
        setLaunchProgress(null)
        setLaunchInstanceId(null)
        const time = new Date().toISOString()
        setLastLaunchError({ message: error.message, time })
        pushConsoleLines([{
          stream: 'stderr',
          line: error.message,
          time,
        }])
        pushToast(error.message, 'error')
        return
      }
      const job = useAppStore.getState().installJobs.find((entry) => entry.taskId === taskId)
      if (job) {
        pushToast(`${job.title}: ${error.message}`, 'error')
        removeInstallJob(taskId)
      }
    })
    void invoke('logs:launcherHistory', undefined)
      .then((history) => {
        if (Array.isArray(history) && history.length > 0) {
          setLauncherLogs(history)
        }
      })
      .catch(() => {})
    const offLauncherLog = window.launcher.on('launcher:log', (incoming) => {
      if (Array.isArray(incoming) && incoming.length > 0) {
        pushLauncherLogs(incoming)
      }
    })
    const deferredLogs: LogLine[] = []
    const flushDeferredLogs = (): void => {
      if (deferredLogs.length === 0) return
      const batch = deferredLogs.splice(0, deferredLogs.length)
      pushConsoleLines(batch)
    }
    const onVisibilityChange = (): void => {
      if (!document.hidden) {
        flushDeferredLogs()
      }
    }
    document.addEventListener('visibilitychange', onVisibilityChange)

    const offLog = window.launcher.on('game:log', (payload) => {
      const incoming = Array.isArray(payload) ? payload : payload.lines
      if (!Array.isArray(incoming) || incoming.length === 0) return
      const extras: LogLine[] = []
      for (const line of incoming) {
        // Game may stop logging once the main menu loads; add a ready marker.
        if (!menuAnnounced && /textures-atlas|Narrator library/i.test(line.line)) {
          menuAnnounced = true
          extras.push(systemLine(t('console.menuReached')))
        }
        // Offline accounts always trigger benign Realms authorization warnings.
        if (!realmsNoted && /Could not authorize you against Realms server|Failed to parse into SignedJWT/i.test(line.line)) {
          realmsNoted = true
          extras.push(systemLine(t('console.realmsIgnored')))
        }
      }
      const combined = extras.length > 0 ? [...incoming, ...extras] : incoming
      if (document.hidden && !window.location.hash.startsWith('#/console')) {
        deferredLogs.push(...combined)
        if (deferredLogs.length > 1_500) {
          deferredLogs.splice(0, deferredLogs.length - 1_500)
        }
        return
      }
      pushConsoleLines(combined)
    })
    const offExit = window.launcher.on('game:exit', ({ code }) => {
      flushDeferredLogs()
      menuAnnounced = false
      realmsNoted = false
      setLaunchTaskId(null)
      setLaunchProgress(null)
      setLaunchInstanceId(null)
      setIsGameRunning(false)
      const time = new Date().toISOString()
      const exitMsg = t('console.exitCode', { code: code === null ? t('common.unknown') : code })
      if (code !== null && code !== 0) {
        setLastLaunchError({ message: exitMsg, time })
      }
      pushConsoleLines([{
        stream: code !== null && code !== 0 ? 'stderr' : 'system',
        line: exitMsg,
        time,
      }])
    })
    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange)
      offProgress()
      offDone()
      offError()
      offLauncherLog()
      offLog()
      offExit()
    }
  }, [pushConsoleLines, pushLauncherLogs, pushToast, queryClient, removeInstallJob, setIsGameRunning, setLastLaunchError, setLaunchInstanceId, setLaunchProgress, setLaunchTaskId, setLauncherLogs, t, upsertInstallJob])

  return (
    <HintProvider>
      <div className={styles.shell} data-testid="app-shell">
        <GlassSidebar />
        <div className={styles.frame}>
          <TitleBar trail={trail} account={account} loading={isLoading} />
          <main className={styles.content}>
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={location.pathname}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{
                  opacity: 0,
                  transition: { duration: 0.1, ease: 'easeIn' },
                }}
                transition={{ duration: 0.15, ease: 'easeOut' }}
                className={styles.pageContainer}
              >
                <Suspense fallback={null}>
                  <Routes location={location}>
                    <Route path="/" element={<Home />} />
                    <Route path="/instances" element={<Instances />} />
                    <Route path="/instances/:id" element={<InstanceDetail />} />
                    <Route path="/browse" element={<Browse />} />
                    <Route path="/skins" element={<SkinSelector />} />
                    <Route path="/screenshots" element={<Screenshots />} />
                    <Route path="/accounts" element={<Accounts />} />
                    <Route path="/settings" element={<Settings />} />
                    <Route path="/console" element={<Console />} />
                    <Route path="*" element={<Navigate to="/" replace />} />
                  </Routes>
                </Suspense>
              </motion.div>
            </AnimatePresence>
          </main>
        </div>
        <Toaster dismissLabel={t('common.dismiss')} />
        <InstallDock />
        <Suspense fallback={null}>
          <CommandPalette />
        </Suspense>
      </div>
    </HintProvider>
  )
}

/** Kerangka jendela: rail kiri, bilah judul, konten, dan toast global. */
export function AppShell(): ReactNode {
  return <ShellLayout />
}
