import type { ReactNode } from 'react'
import { useI18n } from '../../i18n'
import styles from './VersionChannelFilter.module.css'

export interface VersionChannelFilterProps {
  hideRelease: boolean
  hideBeta: boolean
  hideAlpha: boolean
  onToggleRelease: (checked: boolean) => void
  onToggleBeta: (checked: boolean) => void
  onToggleAlpha: (checked: boolean) => void
  className?: string
}

export function VersionChannelFilter({
  hideRelease,
  hideBeta,
  hideAlpha,
  onToggleRelease,
  onToggleBeta,
  onToggleAlpha,
  className,
}: VersionChannelFilterProps): ReactNode {
  const { t } = useI18n()

  return (
    <div className={`${styles.container} ${className || ''}`} role="group" aria-label="Version channel filter">
      <label className={`${styles.checkboxLabel} ${hideRelease ? styles.checkboxLabelActive : ''}`}>
        <input
          type="checkbox"
          checked={hideRelease}
          onChange={(e) => onToggleRelease(e.target.checked)}
          className={styles.checkbox}
        />
        <span>{t('browse.filter.hideRelease')}</span>
      </label>

      <label className={`${styles.checkboxLabel} ${hideBeta ? styles.checkboxLabelActive : ''}`}>
        <input
          type="checkbox"
          checked={hideBeta}
          onChange={(e) => onToggleBeta(e.target.checked)}
          className={styles.checkbox}
        />
        <span>{t('browse.filter.hideBeta')}</span>
      </label>

      <label className={`${styles.checkboxLabel} ${hideAlpha ? styles.checkboxLabelActive : ''}`}>
        <input
          type="checkbox"
          checked={hideAlpha}
          onChange={(e) => onToggleAlpha(e.target.checked)}
          className={styles.checkbox}
        />
        <span>{t('browse.filter.hideAlpha')}</span>
      </label>
    </div>
  )
}
