import { useState } from 'react'
import type { ReactNode } from 'react'
import { Download, Layers, PackagePlus, Sparkles } from 'lucide-react'
import type { ContentProjectSummary } from '@mc/shared'
import { useI18n } from '../../i18n'
import { GlassButton } from '../GlassButton/GlassButton'
import styles from './ModpackCard.module.css'

export interface ModpackCardProps {
  item: ContentProjectSummary
  onInstall: (item: ContentProjectSummary) => void
  installLabel?: string
}

function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(value)
}

export function ModpackCard({
  item,
  onInstall,
  installLabel,
}: ModpackCardProps): ReactNode {
  const { t } = useI18n()
  const [iconBroken, setIconBroken] = useState(false)
  const showIcon = item.iconUrl !== null && !iconBroken

  return (
    <article className={styles.card} data-testid="content-card">
      <div className={styles.topRow}>
        <div className={styles.iconWrapper}>
          {showIcon ? (
            <img
              className={styles.icon}
              src={item.iconUrl ?? ''}
              alt=""
              width={54}
              height={54}
              loading="lazy"
              decoding="async"
              onError={() => setIconBroken(true)}
            />
          ) : (
            <span className={styles.iconFallback} aria-hidden>
              <Layers size={24} />
            </span>
          )}
        </div>

        <div className={styles.titleArea}>
          <div className={styles.badges}>
            <span className={styles.badgeModpack}>
              <Sparkles size={11} aria-hidden />
              {t('browse.modpackBadge')}
            </span>
            <span className={styles.badgeProvider}>{item.provider}</span>
          </div>
          <h3 className={styles.title} title={item.title}>
            {item.title}
          </h3>
          <span className={styles.author}>{t('browse.byAuthor', { author: item.slug })}</span>
        </div>
      </div>

      <p className={styles.description}>{item.description}</p>

      <div className={styles.footer}>
        <div className={styles.meta}>
          <span className={styles.downloads}>
            <Download size={13} aria-hidden />
            {t('browse.downloads', { count: formatCount(item.downloads) })}
          </span>
        </div>

        <GlassButton
          variant="primary"
          size="sm"
          icon={<PackagePlus size={14} aria-hidden />}
          onClick={() => onInstall(item)}
          data-testid="content-install-button"
        >
          {installLabel ?? t('browse.installModpackAction')}
        </GlassButton>
      </div>
    </article>
  )
}
