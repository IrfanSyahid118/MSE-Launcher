import type { ReactNode } from 'react'
import type { MinecraftVersionType } from '@mc/shared'
import styles from './VersionBlockIcon.module.css'

export interface VersionBlockIconProps {
  type: MinecraftVersionType
  size?: number
}

/**
 * Ikon blok isometrik prosedural per tipe versi. Tidak memakai file gambar:
 * tiga bidang SVG diwarnai lewat token tema.
 */
export function VersionBlockIcon({ type, size = 30 }: VersionBlockIconProps): ReactNode {
  return (
    <span className={`${styles.wrapper} ${styles[type] ?? ''}`} aria-hidden>
      <svg width={size} height={size} viewBox="0 0 32 32" focusable="false">
        <path d="M16 3 28 9.6v12.8L16 29 4 22.4V9.6z" className={styles.body} />
        <path d="M16 3 28 9.6 16 16.2 4 9.6z" className={styles.top} />
        <path d="M16 16.2V29l12-6.6V9.6z" className={styles.side} />
        <path d="M16 16.2 4 9.6v12.8L16 29z" className={styles.face} />
      </svg>
    </span>
  )
}
