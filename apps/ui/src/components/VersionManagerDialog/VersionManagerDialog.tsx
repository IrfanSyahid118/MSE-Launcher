import { useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Check, Search } from 'lucide-react'
import type { MinecraftVersionInfo } from '@mc/shared'
import { useI18n } from '../../i18n'
import { GlassDialog } from '../GlassDialog/GlassDialog'
import { GlassButton } from '../GlassButton/GlassButton'
import { ProgressBar } from '../ProgressBar/ProgressBar'
import { VersionBlockIcon } from '../VersionBlockIcon/VersionBlockIcon'
import styles from './VersionManagerDialog.module.css'

export interface VersionManagerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  versions: MinecraftVersionInfo[]
  selectedId: string | null
  loading: boolean
  error: string | null
  onSelect: (versionId: string) => void
}

const ROW_HEIGHT = 52

export function VersionManagerDialog({
  open,
  onOpenChange,
  versions,
  selectedId,
  loading,
  error,
  onSelect,
}: VersionManagerDialogProps): ReactNode {
  const { t, language } = useI18n()
  const [query, setQuery] = useState('')
  const scrollRef = useRef<HTMLDivElement | null>(null)

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) {
      return versions
    }
    return versions.filter((version) => version.id.toLowerCase().includes(needle))
  }, [query, versions])

  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
  })

  const installedCount = versions.filter((version) => version.installed).length
  const typeLabels = {
    release: t('version.type.release'),
    snapshot: t('version.type.snapshot'),
    old_beta: t('version.type.old_beta'),
    old_alpha: t('version.type.old_alpha'),
  } as const

  return (
    <GlassDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('versionManager.title')}
      description={t('versionManager.description')}
      closeLabel={t('common.close')}
      size="lg"
      footer={
        <>
          <span className={styles.footerMeta}>
            {t('version.manager.count', { count: versions.length })} ·{' '}
            {t('version.manager.installedCount', { count: installedCount })}
          </span>
          <GlassButton variant="ghost" onClick={() => onOpenChange(false)}>
            {t('common.close')}
          </GlassButton>
        </>
      }
    >
      <div className={styles.searchRow}>
        <Search size={15} className={styles.searchIcon} aria-hidden />
        <input
          className={styles.search}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('versionManager.search')}
          aria-label={t('versionManager.search')}
          autoFocus
        />
      </div>

      {loading ? (
        <div className={styles.status}>
          <span>{t('common.loading')}</span>
          <ProgressBar label={t('common.loading')} />
        </div>
      ) : error ? (
        <p className={styles.status}>{error}</p>
      ) : filtered.length === 0 ? (
        <p className={styles.status}>{t('versionManager.empty')}</p>
      ) : (
        <div ref={scrollRef} className={styles.scroll} data-testid="version-list">
          <div className={styles.virtualInner} style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((item) => {
              const version = filtered[item.index]
              if (!version) {
                return null
              }
              const selected = version.id === selectedId
              const date = new Date(version.releaseTime)
              const dateLabel = Number.isNaN(date.getTime())
                ? version.releaseTime
                : new Intl.DateTimeFormat(language, { dateStyle: 'medium' }).format(date)
              return (
                <button
                  key={version.id}
                  type="button"
                  className={styles.row}
                  style={{ transform: `translateY(${item.start}px)`, height: ROW_HEIGHT }}
                  onClick={() => {
                    onSelect(version.id)
                    onOpenChange(false)
                  }}
                  aria-pressed={selected}
                >
                  <VersionBlockIcon type={version.type} size={26} />
                  <span className={styles.rowText}>
                    <span className={styles.rowId}>{version.id}</span>
                    <span className={styles.rowMeta}>
                      {typeLabels[version.type]} · {dateLabel}
                    </span>
                  </span>
                  {version.installed ? (
                    <span className={styles.installed}>{t('version.installed')}</span>
                  ) : (
                    <span className={styles.notInstalled}>{t('version.notInstalled')}</span>
                  )}
                  {selected ? (
                    <span className={styles.check}>
                      <Check size={15} aria-hidden />
                      <span className={styles.srOnly}>{t('version.selected')}</span>
                    </span>
                  ) : null}
                </button>
              )
            })}
          </div>
        </div>
      )}
    </GlassDialog>
  )
}
