import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { Command, House, Library, Search, Settings, UserRound } from 'lucide-react'
import { Dialog } from 'radix-ui'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { useNavigate } from 'react-router'
import { useI18n } from '../../i18n'
import styles from './CommandPalette.module.css'

interface PaletteCommand {
  id: string
  label: string
  path: string
  icon: typeof House
}

export function CommandPalette(): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()
  const reducedMotion = useReducedMotion()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setOpen((value) => !value)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  const commands = useMemo<PaletteCommand[]>(() => [
    { id: 'home', label: t('nav.home'), path: '/', icon: House },
    { id: 'instances', label: t('nav.instances'), path: '/instances', icon: Library },
    { id: 'browse', label: t('nav.browse'), path: '/browse', icon: Search },
    { id: 'accounts', label: t('nav.accounts'), path: '/accounts', icon: UserRound },
    { id: 'settings', label: t('nav.settings'), path: '/settings', icon: Settings },
  ], [t])
  const filtered = commands.filter((item) => item.label.toLocaleLowerCase().includes(query.toLocaleLowerCase()))

  const select = (item: PaletteCommand): void => {
    navigate(item.path)
    setOpen(false)
    setQuery('')
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <AnimatePresence>
        {open ? (
          <Dialog.Portal forceMount>
            <Dialog.Overlay asChild>
              <motion.div className={styles.overlay} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} />
            </Dialog.Overlay>
            <Dialog.Content asChild aria-describedby={undefined}>
              <motion.section
                className={styles.content}
                initial={reducedMotion ? { opacity: 0 } : { opacity: 0, y: -12, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: -8, scale: 0.98 }}
                transition={{ type: 'spring', stiffness: 500, damping: 36 }}
              >
                <Dialog.Title className={styles.title}>{t('command.title')}</Dialog.Title>
                <div className={styles.searchRow}>
                  <Search size={17} aria-hidden />
                  <input
                    autoFocus
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder={t('command.searchPlaceholder')}
                    aria-label={t('command.searchAria')}
                  />
                  <kbd>Esc</kbd>
                </div>
                <div className={styles.list} role="listbox" aria-label={t('command.title')}>
                  {filtered.length ? filtered.map((item) => {
                    const Icon = item.icon
                    return <button key={item.id} type="button" role="option" className={styles.item} onClick={() => select(item)}>
                      <Icon size={17} aria-hidden />
                      <span>{item.label}</span>
                    </button>
                  }) : <p className={styles.empty}>{t('command.empty')}</p>}
                </div>
                <footer><Command size={13} aria-hidden /> <span>{t('command.shortcutHint')}</span></footer>
              </motion.section>
            </Dialog.Content>
          </Dialog.Portal>
        ) : null}
      </AnimatePresence>
    </Dialog.Root>
  )
}
