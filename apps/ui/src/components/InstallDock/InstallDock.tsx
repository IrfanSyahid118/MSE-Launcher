import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { CheckCircle, PackagePlus, X } from 'lucide-react'
import type { ProgressEvent } from '@mc/shared'
import { errorMessage, invoke } from '../../api/launcher'
import { useInstances } from '../../api/queries'
import { useI18n } from '../../i18n'
import { useAppStore } from '../../stores/app-store'
import { Spinner } from '../Spinner/Spinner'
import styles from './InstallDock.module.css'

function percentOf(progress: ProgressEvent | null): number {
  if (!progress || progress.total <= 0) return 0
  return Math.max(0, Math.min(100, Math.round((progress.completed / progress.total) * 100)))
}

function DockProgressCard({
  title,
  progress,
  logs,
  cancelLabel,
  consoleAriaLabel,
  onCancel,
}: {
  title: string
  progress: ProgressEvent | null
  logs: string[]
  cancelLabel: string
  consoleAriaLabel: string
  onCancel: (() => void) | null
}): ReactNode {
  const { t } = useI18n()
  const consoleRef = useRef<HTMLDivElement | null>(null)
  const hasProgress = progress !== null && progress.total > 0
  const percent = percentOf(progress)
  const running = percent < 100
  const stageLabel = progress?.stage ?? t('installDock.preparing')
  const statusLabel = hasProgress ? `${title}: ${stageLabel} ${percent}%` : `${title}: ${stageLabel}`
  const reducedMotion = document.documentElement.dataset['motion'] === 'reduced'
  const lastLog = logs[logs.length - 1]

  useEffect(() => {
    const el = consoleRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
  }, [logs.length, lastLog])

  return (
    <motion.li
      layout={!reducedMotion}
      className={styles.job}
      initial={reducedMotion ? false : { opacity: 0, y: 18, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 14, scale: 0.96 }}
      transition={{ duration: reducedMotion ? 0 : 0.2, ease: 'easeOut' }}
      role="status"
      aria-busy={running}
      aria-label={statusLabel}
    >
      <div className={styles.jobTop}>
        {running ? (
          <Spinner size={14} className={styles.busyIcon} />
        ) : (
          <CheckCircle size={14} className={styles.doneIcon} aria-hidden />
        )}
        <span className={styles.jobTitle} title={title}>
          {title}
        </span>
        {onCancel ? (
          <button
            type="button"
            className={styles.cancel}
            onClick={onCancel}
            aria-label={cancelLabel}
            title={cancelLabel}
          >
            <X size={12} aria-hidden />
          </button>
        ) : null}
      </div>

      <div className={styles.jobMeta}>
        <span className={styles.stage} title={stageLabel}>
          {stageLabel}
        </span>
        {hasProgress ? <span className={styles.percent}>{percent}%</span> : null}
      </div>

      <div
        className={styles.track}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <div className={styles.fill} style={{ width: `${percent}%` }} />
      </div>

      {logs.length > 0 ? (
        <div
          ref={consoleRef}
          className={styles.console}
          role="log"
          aria-label={consoleAriaLabel}
        >
          {logs.map((line, index) => {
            const isLatest = index === logs.length - 1
            return (
              <div
                key={`${index}-${line}`}
                className={`${styles.consoleLine} ${isLatest ? styles.consoleLineActive : ''}`}
                title={line}
              >
                <span className={styles.consolePrompt} aria-hidden>
                  {isLatest ? '>' : '·'}
                </span>
                <span className={styles.consoleText}>{line}</span>
              </div>
            )
          })}
        </div>
      ) : null}
    </motion.li>
  )
}

/**
 * Dock progres instalasi dan penyiapan peluncuran di bawah layar: satu kartu
 * ringkas per pekerjaan dengan bar progres tunggal serta log aktivitas konsol.
 */
export function InstallDock(): ReactNode {
  const { t } = useI18n()
  const jobs = useAppStore((state) => state.installJobs)
  const removeInstallJob = useAppStore((state) => state.removeInstallJob)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const launchProgress = useAppStore((state) => state.launchProgress)
  const launchLogs = useAppStore((state) => state.launchLogs)
  const isGameRunning = useAppStore((state) => state.isGameRunning)
  const setLaunchProgress = useAppStore((state) => state.setLaunchProgress)
  const pushToast = useAppStore((state) => state.pushToast)
  const instancesQuery = useInstances()

  const showLaunchJob =
    !isGameRunning &&
    launchProgress !== null &&
    (launchTaskId !== null || launchInstanceId !== null)

  if (jobs.length === 0 && !showLaunchJob) {
    return null
  }

  const launchingInstance = launchInstanceId
    ? (instancesQuery.data ?? []).find((entry) => entry.id === launchInstanceId)
    : undefined
  const launchTitle = launchingInstance
    ? t('installDock.preparingInstance', { name: launchingInstance.name })
    : t('home.launching')

  const cancelLaunch = (): void => {
    if (!launchTaskId || launchTaskId.startsWith('launch-')) return
    const prevProgress = launchProgress
    setLaunchProgress({
      stage: t('installDock.cancelling'),
      detail: t('installDock.cancelling'),
      completed: prevProgress?.completed ?? 0,
      total: prevProgress?.total ?? 1000,
    })
    void invoke('tasks:cancel', { taskId: launchTaskId }).catch((err) => {
      if (prevProgress) {
        setLaunchProgress(prevProgress)
      }
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    })
  }

  return (
    <div className={styles.dock} aria-label={t('installDock.active')}>
      <ul className={styles.list}>
        <AnimatePresence initial={false}>
          {showLaunchJob ? (
            <DockProgressCard
              key={`launch-${launchInstanceId ?? launchTaskId ?? 'active'}`}
              title={launchTitle}
              progress={launchProgress}
              logs={launchLogs}
              cancelLabel={t('installDock.cancelLaunch')}
              consoleAriaLabel={t('installDock.consoleAria')}
              onCancel={launchTaskId && !launchTaskId.startsWith('launch-') ? cancelLaunch : null}
            />
          ) : null}
          {jobs.map((job) => (
            <DockProgressCard
              key={job.taskId}
              title={job.title}
              progress={job.progress}
              logs={job.logs ?? []}
              cancelLabel={t('installDock.cancel')}
              consoleAriaLabel={t('installDock.consoleAria')}
              onCancel={() => {
                void invoke('tasks:cancel', { taskId: job.taskId })
                  .then(() => {
                    removeInstallJob(job.taskId)
                  })
                  .catch((err) => {
                    pushToast(errorMessage(err, t('error.unknown')), 'error')
                  })
              }}
            />
          ))}
        </AnimatePresence>
      </ul>
      <div className={styles.hintRow}>
        <PackagePlus size={11} aria-hidden />
        <span>
          {showLaunchJob && jobs.length === 0
            ? t('installDock.launchRunning')
            : t('installDock.running')}
        </span>
      </div>
    </div>
  )
}
