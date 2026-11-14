import { useState } from 'react'
import type { ReactNode } from 'react'
import { Download, PackagePlus } from 'lucide-react'
import type { ContentProjectSummary } from '@mc/shared'
import { useI18n } from '../../i18n'
import { GlassButton } from '../GlassButton/GlassButton'
import styles from './ContentCard.module.css'

export interface ContentCardProps {
  item: ContentProjectSummary
  onInstall: (item: ContentProjectSummary) => void
  installLabel: string
}

function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(value)
}

export function ContentCard({ item, onInstall, installLabel }: ContentCardProps): ReactNode {
  const { t } = useI18n()
  const [iconBroken, setIconBroken] = useState(false)
  const showIcon = item.iconUrl !== null && !iconBroken

  return (
    <article className={styles.card} data-testid="content-card">
      <div className={styles.thumbnailWrap}>
        {showIcon ? (
          <img
            className={styles.icon}
            src={item.iconUrl ?? ''}
            alt=""
            width={60}
            height={60}
            loading="lazy"
            decoding="async"
            onError={() => setIconBroken(true)}
          />
        ) : (
          <span className={styles.iconFallback} aria-hidden>
            {item.title.slice(0, 1).toUpperCase()}
          </span>
        )}
      </div>

      <div className={styles.details}>
        <div className={styles.head}>
          <h3 className={styles.title} title={item.title}>
            {item.title}
          </h3>
          <span className={styles.author}>by {item.slug}</span>
        </div>

        <p className={styles.description}>{item.description}</p>

        <div className={styles.tags}>
          <span className={styles.tag}>{item.type.toUpperCase()}</span>
          <span className={styles.tagProvider}>{item.provider}</span>
        </div>
      </div>

      <div className={styles.actions}>
        <GlassButton
          variant="outline"
          icon={<PackagePlus size={14} aria-hidden />}
          onClick={() => onInstall(item)}
          data-testid="content-install-button"
        >
          {installLabel}
        </GlassButton>

        <div className={styles.metrics}>
          <span className={styles.downloads}>
            <Download size={12} aria-hidden />
            {t('browse.downloads', { count: formatCount(item.downloads) })}
          </span>
        </div>
      </div>
    </article>
  )
}
