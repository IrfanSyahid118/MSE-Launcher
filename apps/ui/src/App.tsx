import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { HashRouter } from 'react-router'
import { RefreshCw } from 'lucide-react'
import { I18nProvider, useI18n } from './i18n'
import { useSettings } from './api/queries'
import { errorMessage, invoke } from './api/launcher'
import { useThemeEffect } from './hooks/useThemeEffect'
import { AppShell } from './components/AppShell/AppShell'
import { ErrorBoundary } from './components/ErrorBoundary/ErrorBoundary'
import { GlassButton } from './components/GlassButton/GlassButton'
import styles from './App.module.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      gcTime: 30_000,
    },
  },
})

function BootMark(): ReactNode {
  return (
    <span className={styles.mark} aria-hidden>
      <svg viewBox="0 0 32 32" width="34" height="34" focusable="false">
        <path d="M16 3 28 9.6v12.8L16 29 4 22.4V9.6z" className={styles.markBody} />
        <path d="M16 3 28 9.6 16 16.2 4 9.6z" className={styles.markTop} />
        <path d="M16 16.2V29l12-6.6V9.6z" className={styles.markSide} />
      </svg>
    </span>
  )
}

function Bootstrap(): ReactNode {
  const { t } = useI18n()
  const settingsQuery = useSettings()
  useThemeEffect(settingsQuery.data)

  if (settingsQuery.isLoading) {
    return (
      <div className={styles.boot}>
        <BootMark />
        <p className={styles.bootText}>{t('boot.loading')}</p>
      </div>
    )
  }

  if (settingsQuery.isError || !settingsQuery.data) {
    return (
      <div className={styles.boot}>
        <BootMark />
        <h1 className={styles.bootTitle}>{t('boot.errorTitle')}</h1>
        <p className={styles.bootText}>{t('boot.errorHint')}</p>
        <p className={styles.bootError}>
          {errorMessage(settingsQuery.error, t('error.loadFailed'))}
        </p>
        <GlassButton
          variant="primary"
          icon={<RefreshCw size={14} aria-hidden />}
          onClick={() => {
            void settingsQuery.refetch()
          }}
        >
          {t('common.retry')}
        </GlassButton>
      </div>
    )
  }

  return (
    <HashRouter>
      <AppShell />
    </HashRouter>
  )
}

function LocalizedApp(): ReactNode {
  const settingsQuery = useSettings()

  return (
    <I18nProvider language={settingsQuery.data?.language ?? 'id'}>
      <CrashBoundary />
    </I18nProvider>
  )
}

function CrashBoundary(): ReactNode {
  const { t } = useI18n()

  return (
    <ErrorBoundary
      onError={(error) => {
        void invoke('logs:appendClient', {
          stream: 'stderr',
          line: `Renderer crash: ${error.name}: ${error.message}`,
        }).catch(() => {})
      }}
      fallback={
        <div className={styles.boot} role="alert">
          <BootMark />
          <h1 className={styles.bootTitle}>{t('boot.crashedTitle')}</h1>
          <p className={styles.bootText}>{t('boot.crashedHint')}</p>
          <GlassButton
            variant="primary"
            icon={<RefreshCw size={14} aria-hidden />}
            onClick={() => {
              window.location.reload()
            }}
          >
            {t('common.reload')}
          </GlassButton>
        </div>
      }
    >
      <Bootstrap />
    </ErrorBoundary>
  )
}

export function App(): ReactNode {
  return (
    <QueryClientProvider client={queryClient}>
      <LocalizedApp />
    </QueryClientProvider>
  )
}
