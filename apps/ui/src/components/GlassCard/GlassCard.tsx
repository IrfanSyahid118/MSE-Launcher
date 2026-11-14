import type { ReactNode } from 'react'
import styles from './GlassCard.module.css'

export type GlassTone = 'glass' | 'strong' | 'quiet'

const toneClass: Record<GlassTone, string> = {
  glass: 'glass',
  strong: 'glassStrong',
  quiet: 'glassQuiet',
}

export interface GlassCardProps {
  children: ReactNode
  tone?: GlassTone
  className?: string | undefined
  padded?: boolean
  'data-testid'?: string | undefined
}

export function GlassCard({
  children,
  tone = 'glass',
  className,
  padded = true,
  'data-testid': testId,
}: GlassCardProps): ReactNode {
  return (
    <div
      data-testid={testId}
      className={[toneClass[tone], styles.card, padded ? styles.padded : '', className ?? '']
        .filter(Boolean)
        .join(' ')}
    >
      {children}
    </div>
  )
}
