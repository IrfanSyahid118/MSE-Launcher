import type { ReactNode } from 'react'
import { Play, Settings2 } from 'lucide-react'
import { useNavigate } from 'react-router'
import type { Instance } from '@mc/shared'
import { useI18n } from '../../i18n'
import { InstanceIcon } from '../InstanceIcon/InstanceIcon'
import { Spinner } from '../Spinner/Spinner'
import styles from './InstanceCard.module.css'

export interface InstanceCardProps {
  instance: Instance
  onLaunch(instance: Instance): void
  isLaunching?: boolean
}

function loaderBadge(instance: Instance): string {
  if (instance.loader.type === 'vanilla') return 'Vanilla'
  return instance.loader.version
    ? `${instance.loader.type.toUpperCase()} ${instance.loader.version}`
    : instance.loader.type.toUpperCase()
}

/**
 * Kartu instance. Menampilkan icon modpack creator jika instance merupakan modpack,
 * atau icon default grass block untuk instance standar/vanilla.
 */
export function InstanceCard({ instance, onLaunch, isLaunching }: InstanceCardProps): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()

  return (
    <article className={styles.card} data-testid={`instance-card-${instance.id}`}>
      <a
        href={`#/instances/${instance.id}`}
        className={styles.cardLink}
        aria-label={`${instance.name}, ${t('instances.detail')}`}
        onClick={(event) => {
          event.preventDefault()
          navigate(`/instances/${instance.id}`)
        }}
      />

      <InstanceIcon instance={instance} size={36} className={styles.icon} />

      <div className={styles.body}>
        <h3 className={styles.name} title={instance.name}>
          {instance.name}
        </h3>
        <p className={styles.subtitle}>
          {loaderBadge(instance)} {instance.minecraftVersion}
        </p>
      </div>

      <div className={styles.actions}>
        {/* Tombol pertama membuka detail, supaya alurnya sama untuk pengguna keyboard. */}
        <button
          type="button"
          className={styles.iconButton}
          onClick={() => navigate(`/instances/${instance.id}`)}
          title={t('instances.detail')}
          aria-label={t('instances.detail')}
        >
          <Settings2 size={15} aria-hidden />
        </button>
        <button
          type="button"
          className={`${styles.actionButton} ${styles.playButton}`}
          onClick={() => onLaunch(instance)}
          disabled={isLaunching}
          aria-label={t('instances.play')}
          title={t('instances.play')}
        >
          {isLaunching ? (
            <Spinner size={14} />
          ) : (
            <Play size={14} fill="currentColor" aria-hidden />
          )}
          <span>{isLaunching ? t('home.launching') : t('instances.play')}</span>
        </button>
      </div>
    </article>
  )
}
