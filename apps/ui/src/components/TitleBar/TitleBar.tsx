import type { ReactNode } from 'react'
import { Link, useNavigate } from 'react-router'
import { ChevronLeft, ChevronRight, Download, RefreshCw } from 'lucide-react'
import type { Account, Instance } from '@mc/shared'
import { useI18n } from '../../i18n'
import { useInstances } from '../../api/queries'
import { useAppStore } from '../../stores/app-store'
import type { BreadcrumbSegment } from '../../stores/app-store'
import { AccountMenu } from '../AccountMenu/AccountMenu'
import styles from './TitleBar.module.css'

export interface TitleBarProps {
  /** Jejak halaman, segmen terakhir adalah halaman yang sedang dibuka. */
  trail: BreadcrumbSegment[]
  account: Account | null
  loading: boolean
}

function loaderLabel(instance: Instance | undefined): string | null {
  if (!instance) return null
  const loader =
    instance.loader.type === 'vanilla' ? 'Vanilla' : instance.loader.type.toUpperCase()
  return `${loader} ${instance.minecraftVersion}`
}

/** Bilah judul: merek, riwayat navigasi, status unduhan, dan akun aktif. */
export function TitleBar({ trail, account, loading }: TitleBarProps): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()
  const isGameRunning = useAppStore((state) => state.isGameRunning)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchProgress = useAppStore((state) => state.launchProgress)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const selectedInstanceId = useAppStore((state) => state.selectedInstanceId)
  const instancesQuery = useInstances()

  const instances = instancesQuery.data ?? []
  const activeInstance =
    instances.find((instance) => instance.id === (launchInstanceId ?? selectedInstanceId)) ??
    instances[0]
  const activeLabel = loaderLabel(activeInstance)
  const percent =
    launchProgress?.total && launchProgress.total > 0
      ? Math.round((launchProgress.completed / launchProgress.total) * 100)
      : null

  return (
    <header className={styles.titlebar}>
      <div className={styles.left}>
        <Link to="/" className={styles.brand} aria-label={t('app.name')}>
          <span className={styles.brandIcon} aria-hidden>
            <svg viewBox="0 0 32 32" width="19" height="19" focusable="false">
              <circle cx="16" cy="16" r="14" fill="#00af5c" />
              <path d="M16 6.5 25 11.7v8.6L16 25.5 7 20.3v-8.6z" fill="#ffffff" opacity="0.92" />
              <path d="M16 6.5 25 11.7 16 16.9 7 11.7z" fill="#c9f6dd" />
            </svg>
          </span>
          <span className={styles.brandName}>{t('app.name')}</span>
        </Link>

        <div className={styles.historyControls}>
          <button
            type="button"
            className={styles.historyBtn}
            onClick={() => navigate(-1)}
            aria-label={t('titlebar.back')}
            title={t('titlebar.back')}
            data-testid="titlebar-back"
          >
            <ChevronLeft size={16} aria-hidden />
          </button>
          <button
            type="button"
            className={styles.historyBtn}
            onClick={() => navigate(1)}
            aria-label={t('titlebar.forward')}
            title={t('titlebar.forward')}
          >
            <ChevronRight size={16} aria-hidden />
          </button>
        </div>

        <nav className={styles.trail} aria-label="Breadcrumb">
          <svg className={styles.trailCaret} viewBox="0 0 12 12" aria-hidden focusable="false">
            <path d="M3 1.6 9.2 6 3 10.4z" fill="currentColor" />
          </svg>
          {trail.map((segment, index) => {
            const isLast = index === trail.length - 1
            return (
              <span key={`${segment.label}-${index}`} className={styles.trailItem}>
                {index > 0 ? (
                  <span className={styles.trailSeparator} aria-hidden>
                    ›
                  </span>
                ) : null}
                {isLast ? (
                  <span
                    className={styles.trailCurrent}
                    aria-current="page"
                    data-testid="screen-title"
                  >
                    {segment.label}
                  </span>
                ) : segment.onClick ? (
                  <button
                    type="button"
                    className={styles.trailLink}
                    onClick={segment.onClick}
                  >
                    {segment.label}
                  </button>
                ) : segment.to ? (
                  <Link to={segment.to} className={styles.trailLink}>
                    {segment.label}
                  </Link>
                ) : (
                  <span className={styles.trailCurrent}>{segment.label}</span>
                )}
              </span>
            )
          })}
        </nav>
      </div>

      <div className={styles.right}>
        {activeLabel ? (
          <div
            className={`${styles.loaderPill} ${launchTaskId ? styles.loaderPillActive : ''}`}
            title={
              launchTaskId
                ? launchProgress?.stage ?? t('titlebar.noInstancesRunning')
                : activeInstance?.name
            }
          >
            <svg className={styles.loaderMark} viewBox="0 0 16 16" aria-hidden focusable="false">
              <path
                d="M8 1.6 13.6 4.8v6.4L8 14.4 2.4 11.2V4.8z"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinejoin="round"
              />
              <path d="M8 1.6 13.6 4.8 8 8 2.4 4.8z" fill="currentColor" opacity="0.35" />
            </svg>
            <span className={styles.loaderText}>{activeLabel}</span>
            {launchTaskId && percent !== null ? (
              <>
                <span className={styles.loaderSpeed}>{percent}%</span>
                <span className={styles.downloadBadge}>
                  <Download size={11} aria-hidden />
                </span>
              </>
            ) : null}
          </div>
        ) : null}

        <button
          type="button"
          className={styles.updatePill}
          onClick={() => window.location.reload()}
          title="Reload interface"
        >
          <RefreshCw size={13} className={styles.updateIcon} aria-hidden />
          <span>Reload</span>
        </button>

        <div
          className={`${styles.statusPill} ${isGameRunning ? styles.statusPillActive : ''}`}
          role="status"
          aria-live="polite"
        >
          <span
            className={`${styles.statusDot} ${isGameRunning ? styles.statusDotActive : ''}`}
            aria-hidden
          />
          <span className={styles.statusText}>
            {isGameRunning ? t('titlebar.instanceRunning') : t('titlebar.noInstancesRunning')}
          </span>
        </div>

        {loading ? (
          <span className={styles.loading}>{t('common.loading')}</span>
        ) : account ? (
          <AccountMenu
            trigger={
              <button type="button" className={styles.account} aria-label={t('titlebar.account')}>
                <span className={styles.avatar} aria-hidden>
                  {account.username.slice(0, 2).toUpperCase()}
                </span>
                <span className={styles.accountText}>
                  <span className={styles.username}>{account.username}</span>
                  <span className={styles.accountType}>
                    {account.type === 'offline'
                      ? t('account.type.offline')
                      : t('account.type.microsoft')}
                  </span>
                </span>
              </button>
            }
          />
        ) : (
          <AccountMenu
            trigger={
              <button type="button" className={styles.account} aria-label={t('account.create')}>
                <span className={`${styles.avatar} ${styles.avatarEmpty}`} aria-hidden>
                  +
                </span>
                <span className={styles.accountText}>
                  <span className={styles.username}>{t('account.none')}</span>
                  <span className={styles.accountType}>{t('account.create')}</span>
                </span>
              </button>
            }
          />
        )}
      </div>
    </header>
  )
}
