import { Tooltip } from 'radix-ui'
import type { ReactNode } from 'react'
import styles from './Hint.module.css'

export type HintSide = 'top' | 'right' | 'bottom' | 'left'

export interface HintProps {
  content: ReactNode
  side?: HintSide
  delayDuration?: number
  children: ReactNode
}

/** Tooltip tipis untuk label yang tidak muat atau kontrol nonaktif. */
export function Hint({
  content,
  side = 'top',
  delayDuration = 250,
  children,
}: HintProps): ReactNode {
  if (!content) {
    return <>{children}</>
  }

  return (
    <Tooltip.Root delayDuration={delayDuration}>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={styles.content} side={side} sideOffset={8} collisionPadding={12}>
          {content}
          <Tooltip.Arrow className={styles.arrow} width={10} height={5} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

export function HintProvider({ children }: { children: ReactNode }): ReactNode {
  return <Tooltip.Provider delayDuration={250}>{children}</Tooltip.Provider>
}
