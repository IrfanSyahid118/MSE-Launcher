import type { ReactNode } from 'react'
import { useI18n } from '../../i18n'
import styles from './ProgressBar.module.css'

export interface ProgressBarProps {
  /** 0‑100; if undefined shows indeterminate animation */
  percent?: number | undefined
  /** optional label shown above the bar */
  label?: string
  /** optional secondary text or detail */
  detail?: string
}

export function ProgressBar({ percent, label, detail }: ProgressBarProps): ReactNode {
  const { t } = useI18n()
  const displayLabel = label ?? t('common.loading')
  const indeterminate = percent == null
  const clampedPercent = percent != null ? Math.max(0, Math.min(100, Math.round(percent))) : undefined

  return (
    <div className={styles.container} data-testid="progress-bar">
      <div className={styles.header}>
        <span className={styles.label} title={displayLabel}>
          {displayLabel}
        </span>
        <div className={styles.meta}>
          {detail ? <span className={styles.detail}>{detail}</span> : null}
          {clampedPercent != null ? (
            <span className={styles.percentText}>{clampedPercent}%</span>
          ) : null}
        </div>
      </div>

      <div
        className={styles.track}
        role="progressbar"
        aria-busy={indeterminate}
        {...(clampedPercent !== undefined
          ? { 'aria-valuenow': clampedPercent, 'aria-valuemin': 0, 'aria-valuemax': 100 }
          : {})}
      >
        <div
          className={`${styles.fill} ${indeterminate ? styles.indeterminate : ''}`}
          style={clampedPercent !== undefined ? { width: `${clampedPercent}%` } : undefined}
        />
      </div>
    </div>
  )
}
