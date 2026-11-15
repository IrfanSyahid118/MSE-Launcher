import type { CleanupSummary, LauncherErrorData } from '@mc/shared'
import type { QueryClient } from '@tanstack/react-query'
import type { Translate } from '../i18n'
import { consumeBufferedTaskEvent, useAppStore } from '../stores/app-store'
import { queryKeys } from './queries'

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

export function handleCleanupTaskDone(
  result: unknown,
  t: Translate,
): void {
  const { setCleanupTaskId, pushToast } = useAppStore.getState()
  setCleanupTaskId(null)
  const summary = result as CleanupSummary | undefined
  pushToast(
    t('instances.cleanupDone', {
      files: summary?.removedFiles ?? 0,
      size: formatBytes(summary?.freedBytes ?? 0),
    }),
  )
}

export function handleLaunchTaskDone(queryClient: QueryClient): void {
  const { setLaunchTaskId, setLaunchProgress, setIsGameRunning } = useAppStore.getState()
  setLaunchTaskId(null)
  setLaunchProgress(null)
  setIsGameRunning(true)
  void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
}

export function handleLaunchTaskError(error: LauncherErrorData): void {
  const {
    setLaunchTaskId,
    setLaunchProgress,
    setLaunchInstanceId,
    setLastLaunchError,
    pushConsoleLines,
    pushToast,
  } = useAppStore.getState()
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
}

export function registerLaunchTask(taskId: string, queryClient: QueryClient): void {
  const store = useAppStore.getState()
  const buffered = consumeBufferedTaskEvent(taskId)
  if (buffered?.error) {
    handleLaunchTaskError(buffered.error)
    return
  }
  if (buffered?.done) {
    handleLaunchTaskDone(queryClient)
    return
  }
  store.setLaunchTaskId(taskId)
  if (buffered?.progress) {
    store.setLaunchProgress(buffered.progress)
  }
  if (buffered?.logs && buffered.logs.length > 0) {
    store.restoreLaunchLogs(buffered.logs)
  }
}

export function registerCleanupTask(taskId: string, t: Translate): void {
  const store = useAppStore.getState()
  const buffered = consumeBufferedTaskEvent(taskId)
  if (buffered?.error) {
    store.setCleanupTaskId(null)
    store.pushToast(buffered.error.message, 'error')
    return
  }
  if (buffered?.done) {
    handleCleanupTaskDone(buffered.done.result, t)
    return
  }
  store.setCleanupTaskId(taskId)
}

export function registerInstallTask(
  taskId: string,
  title: string,
  queryClient: QueryClient,
  t: Translate,
): 'active' | 'done' | 'error' {
  const store = useAppStore.getState()
  const buffered = consumeBufferedTaskEvent(taskId)
  if (buffered?.error) {
    store.pushToast(`${title}: ${buffered.error.message}`, 'error')
    return 'error'
  }
  if (buffered?.done) {
    void queryClient.invalidateQueries({ queryKey: queryKeys.instances })
    store.pushToast(t('browse.installDone', { title }))
    return 'done'
  }
  store.upsertInstallJob({
    taskId,
    title,
    progress: buffered?.progress ?? null,
    ...(buffered?.logs ? { logs: buffered.logs } : {}),
  })
  return 'active'
}
