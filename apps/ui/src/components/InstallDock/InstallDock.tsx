import { AnimatePresence, motion } from 'motion/react'
import type { ReactNode } from 'react'
import { CheckCircle, PackagePlus, X } from 'lucide-react'
import { invoke } from '../../api/launcher'
import { useI18n } from '../../i18n'
import { useAppStore } from '../../stores/app-store'
import { Spinner } from '../Spinner/Spinner'
import styles from './InstallDock.module.css'

function percentOf(progress: { completed: number; total: number } | null): number {
  if (!progress || progress.total <= 0) return 0
  return Math.max(0, Math.min(100, Math.round((progress.completed / progress.total) * 100)))
}

function InstallJobCard({ taskId, title, progress }: {
  taskId: string
  title: string
  progress: ReturnType<typeof useAppStore.getState>['installJobs'][number]['progress']
}): ReactNode {
  const { t } = useI18n()
  const removeInstallJob = useAppStore((state) => state.removeInstallJob)
  const hasProgress = progress !== null && progress.total > 0
  const percent = percentOf(progress)
  const running = percent < 100
  const stageLabel = progress?.stage ?? t('installDock.preparing')
  const statusLabel = hasProgress ? `${title}: ${stageLabel} ${percent}%` : `${title}: ${stageLabel}`
  const reducedMotion = document.documentElement.dataset['motion'] === 'reduced'
  const cancel = (): void => {
    void invoke('tasks:cancel', { taskId })
      .catch(() => undefined)
      .finally(() => {
        removeInstallJob(taskId)
      })
  }

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
        <button
          type="button"
          className={styles.cancel}
          onClick={cancel}
          aria-label={t('installDock.cancel')}
          title={t('installDock.cancel')}
        >
          <X size={12} aria-hidden />
        </button>
      </div>

      <div className={styles.jobMeta}>
        <span className={styles.stage}>{progress?.stage ?? t('installDock.preparing')}</span>
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
    </motion.li>
  )
}

/**
 * Dock progres instalasi di bawah layar, seperti dock macOS: satu kartu ringkas
 * per pekerjaan unduhan, dengan persen tunggal 0-100% tanpa melompat mundur.
 */
export function InstallDock(): ReactNode {
  const { t } = useI18n()
  const jobs = useAppStore((state) => state.installJobs)

  if (jobs.length === 0) {
    return null
  }

  return (
    <div className={styles.dock} aria-label={t('installDock.active')}>
      <ul className={styles.list}>
        <AnimatePresence initial={false}>
          {jobs.map((job) => (
            <InstallJobCard
              key={job.taskId}
              taskId={job.taskId}
              title={job.title}
              progress={job.progress}
            />
          ))}
        </AnimatePresence>
      </ul>
      <div className={styles.hintRow}>
        <PackagePlus size={11} aria-hidden />
        <span>{t('installDock.running')}</span>
      </div>
    </div>
  )
}
