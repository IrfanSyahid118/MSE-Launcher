import { useEffect } from 'react'
import type { ReactNode } from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { AlertCircle, Info, X } from 'lucide-react'
import { useAppStore } from '../../stores/app-store'
import { useI18n } from '../../i18n'
import type { ToastItem } from '../../stores/app-store'
import styles from './Toaster.module.css'

const AUTO_DISMISS_MS = 4000

interface ToastCardProps {
  toast: ToastItem
  dismissLabel: string
}

function ToastCard({ toast, dismissLabel }: ToastCardProps): ReactNode {
  const dismissToast = useAppStore((state) => state.dismissToast)
  const prefersReduced = useReducedMotion()
  const reducedMotion = prefersReduced || document.documentElement.dataset['motion'] === 'reduced'

  useEffect(() => {
    const timer = window.setTimeout(() => {
      dismissToast(toast.id)
    }, AUTO_DISMISS_MS)
    return () => {
      window.clearTimeout(timer)
    }
  }, [dismissToast, toast.id])

  const Icon = toast.tone === 'error' ? AlertCircle : Info

  return (
    <motion.li
      layout={!reducedMotion}
      className={`${styles.toast} ${styles[toast.tone]}`}
      initial={reducedMotion ? false : { opacity: 0, y: 14, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 10, scale: 0.98 }}
      transition={{ duration: reducedMotion ? 0 : 0.18, ease: 'easeOut' }}
      role="status"
      aria-live="polite"
    >
      <Icon size={16} className={styles.icon} aria-hidden />
      <span className={styles.message}>{toast.message}</span>
      <button
        type="button"
        className={styles.dismiss}
        onClick={() => dismissToast(toast.id)}
        aria-label={dismissLabel}
      >
        <X size={14} aria-hidden />
      </button>
    </motion.li>
  )
}

export interface ToasterProps {
  dismissLabel: string
}

/** Antrean toast dari Zustand, auto-dismiss 4 detik, animasi masuk/keluar. */
export function Toaster({ dismissLabel }: ToasterProps): ReactNode {
  const toasts = useAppStore((state) => state.toasts)
  const { t } = useI18n()

  return (
    <ul className={styles.stack} aria-label={t('common.notifications')}>
      <AnimatePresence initial={false}>
        {toasts.map((toast) => (
          <ToastCard key={toast.id} toast={toast} dismissLabel={dismissLabel} />
        ))}
      </AnimatePresence>
    </ul>
  )
}
