import type { ReactNode } from 'react'
import styles from './ScreenHeader.module.css'

export interface ScreenHeaderProps {
  title: string
  subtitle?: string | undefined
  eyebrow?: string | undefined
  actions?: ReactNode
  back?: ReactNode
  titleId?: string | undefined
  testId?: string | undefined
}

/**
 * Judul layar bersama. Semua layar memakai satu ritme vertikal dan satu skala
 * judul, sehingga perpindahan rute tidak terasa seperti aplikasi berbeda.
 */
export function ScreenHeader({
  title,
  subtitle,
  eyebrow,
  actions,
  back,
  titleId,
  testId,
}: ScreenHeaderProps): ReactNode {
  return (
    <header className={styles.header}>
      {back ? <div className={styles.back}>{back}</div> : null}
      <div className={styles.row}>
        <div className={styles.titles}>
          {eyebrow ? <p className={styles.eyebrow}>{eyebrow}</p> : null}
          <h1 className={styles.title} id={titleId} data-testid={testId}>
            {title}
          </h1>
          {subtitle ? <p className={styles.subtitle}>{subtitle}</p> : null}
        </div>
        {actions ? <div className={styles.actions}>{actions}</div> : null}
      </div>
    </header>
  )
}
