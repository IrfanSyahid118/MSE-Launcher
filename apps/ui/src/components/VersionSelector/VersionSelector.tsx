import { useMemo } from 'react'
import type { ReactNode } from 'react'
import { Check, Settings2 } from 'lucide-react'
import type { MinecraftVersionInfo } from '@mc/shared'
import { useI18n } from '../../i18n'
import { VersionBlockIcon } from '../VersionBlockIcon/VersionBlockIcon'
import styles from './VersionSelector.module.css'

export interface VersionSelectorProps {
  versions: MinecraftVersionInfo[]
  recent: MinecraftVersionInfo[]
  other: MinecraftVersionInfo[]
  selectedId: string | null
  onSelect: (versionId: string) => void
  onOpenManager: () => void
}

function formatDate(iso: string, language: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) {
    return iso
  }
  return new Intl.DateTimeFormat(language, { dateStyle: 'medium' }).format(date)
}

interface VersionRowProps {
  version: MinecraftVersionInfo
  selected: boolean
  language: string
  installedLabel: string
  selectedLabel: string
  onSelect: (versionId: string) => void
  typeLabel: string
}

function VersionRow({
  version,
  selected,
  language,
  installedLabel,
  selectedLabel,
  onSelect,
  typeLabel,
}: VersionRowProps): ReactNode {
  return (
    <button
      type="button"
      className={styles.row}
      onClick={() => onSelect(version.id)}
      aria-pressed={selected}
      data-version-id={version.id}
    >
      <VersionBlockIcon type={version.type} />
      <span className={styles.rowText}>
        <span className={styles.rowId}>{version.id}</span>
        <span className={styles.rowMeta}>
          {typeLabel} · {formatDate(version.releaseTime, language)}
        </span>
      </span>
      {version.installed ? (
        <span className={styles.installedBadge}>{installedLabel}</span>
      ) : null}
      {selected ? (
        <span className={styles.check} title={selectedLabel}>
          <Check size={16} aria-hidden />
          <span className={styles.srOnly}>{selectedLabel}</span>
        </span>
      ) : null}
    </button>
  )
}

/** Panel pemilih versi: bagian terbaru, versi lain, dan pintu ke version manager. */
export function VersionSelector({
  versions,
  recent,
  other,
  selectedId,
  onSelect,
  onOpenManager,
}: VersionSelectorProps): ReactNode {
  const { t, language } = useI18n()

  const typeLabels = useMemo(
    () => ({
      release: t('version.type.release'),
      snapshot: t('version.type.snapshot'),
      old_beta: t('version.type.old_beta'),
      old_alpha: t('version.type.old_alpha'),
    }),
    [t],
  )

  if (versions.length === 0) {
    return (
      <div className={styles.panel} data-testid="version-selector">
        <p className={styles.empty}>{t('home.versions.empty')}</p>
      </div>
    )
  }

  return (
    <div className={styles.panel} data-testid="version-selector">
      <div className={styles.summary}>
        <span>{t('version.manager.count', { count: versions.length })}</span>
        <span className={styles.summaryDot} aria-hidden />
        <span>{t('version.manager.installedCount', { count: versions.filter((v) => v.installed).length })}</span>
      </div>

      {recent.length > 0 ? (
        <section className={styles.section}>
          <h3 className={styles.sectionLabel}>{t('home.section.recent')}</h3>
          <div className={styles.list}>
            {recent.map((version) => (
              <VersionRow
                key={version.id}
                version={version}
                selected={version.id === selectedId}
                language={language}
                installedLabel={t('version.installed')}
                selectedLabel={t('version.selected')}
                onSelect={onSelect}
                typeLabel={typeLabels[version.type]}
              />
            ))}
          </div>
        </section>
      ) : null}

      {other.length > 0 ? (
        <section className={styles.section}>
          <h3 className={styles.sectionLabel}>{t('home.section.otherVersions')}</h3>
          <div className={styles.list}>
            {other.map((version) => (
              <VersionRow
                key={version.id}
                version={version}
                selected={version.id === selectedId}
                language={language}
                installedLabel={t('version.installed')}
                selectedLabel={t('version.selected')}
                onSelect={onSelect}
                typeLabel={typeLabels[version.type]}
              />
            ))}
          </div>
        </section>
      ) : null}

      <button type="button" className={styles.manage} onClick={onOpenManager}>
        <Settings2 size={15} aria-hidden />
        {t('home.versionManager')}
      </button>
    </div>
  )
}
