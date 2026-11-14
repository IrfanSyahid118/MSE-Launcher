import type { CSSProperties, ReactNode } from 'react'
import { Loader2 } from 'lucide-react'
import styles from './Spinner.module.css'

export interface SpinnerProps {
  /** Sisi ikon dalam piksel. */
  size?: number | undefined
  className?: string | undefined
  style?: CSSProperties | undefined
}

/**
 * Ikon status "sedang bekerja". Satu-satunya sumber animasi loading di UI,
 * supaya kelas animasi tidak lagi ditulis ulang per komponen.
 */
export function Spinner({ size = 14, className, style }: SpinnerProps): ReactNode {
  return (
    <Loader2
      size={size}
      className={[styles.spinner, className ?? ''].filter(Boolean).join(' ')}
      style={style}
      aria-hidden
    />
  )
}
