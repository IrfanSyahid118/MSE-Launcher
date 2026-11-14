import { Dialog } from 'radix-ui'
import type { ReactNode } from 'react'
import { X } from 'lucide-react'
import styles from './GlassDialog.module.css'

export type GlassDialogSize = 'sm' | 'md' | 'lg' | 'xl'

export interface GlassDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  closeLabel: string
  children: ReactNode
  footer?: ReactNode
  size?: GlassDialogSize
}

/**
 * Dialog kaca dengan portal, fokus terkunci, dan transisi halus. Semua teks
 * datang dari pemanggil supaya tetap melewati i18n.
 */
export function GlassDialog({
  open,
  onOpenChange,
  title,
  description,
  closeLabel,
  children,
  footer,
  size = 'md',
}: GlassDialogProps): ReactNode {
  const contentProps = description ? {} : { 'aria-describedby': undefined }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content className={`${styles.content} ${styles[size]}`} {...contentProps}>
          <header className={styles.header}>
            <div className={styles.heading}>
              <Dialog.Title className={styles.title}>{title}</Dialog.Title>
              {description ? (
                <Dialog.Description className={styles.description}>
                  {description}
                </Dialog.Description>
              ) : null}
            </div>
            <Dialog.Close className={styles.close} aria-label={closeLabel}>
              <X size={18} aria-hidden />
            </Dialog.Close>
          </header>
          <div className={styles.body}>{children}</div>
          {footer ? <footer className={styles.footer}>{footer}</footer> : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
