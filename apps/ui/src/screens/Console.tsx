import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode, UIEvent } from 'react'
import type { DiagnosticReportResult } from '@mc/shared'
import {
  AlertTriangle,
  ArrowDown,
  Copy,
  Download,
  FileCode2,
  FolderOpen,
  Square,
  Trash2,
  X,
} from 'lucide-react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useI18n } from '../i18n'
import { errorMessage, invoke } from '../api/launcher'
import { useAppStore } from '../stores/app-store'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { GlassDialog } from '../components/GlassDialog/GlassDialog'
import { ScreenHeader } from '../components/ScreenHeader/ScreenHeader'
import styles from './Console.module.css'

type StreamFilter = 'all' | 'stdout' | 'stderr' | 'system'
type LogSource = 'minecraft' | 'launcher'

export function Console(): ReactNode {
  const { t } = useI18n()
  const gameLines = useAppStore((state) => state.consoleLines)
  const launcherLines = useAppStore((state) => state.launcherLogs)
  const lastLaunchError = useAppStore((state) => state.lastLaunchError)
  const setLastLaunchError = useAppStore((state) => state.setLastLaunchError)
  const clearConsole = useAppStore((state) => state.clearConsole)
  const clearLauncherLogs = useAppStore((state) => state.clearLauncherLogs)
  const setLauncherLogs = useAppStore((state) => state.setLauncherLogs)
  const isRunning = useAppStore((state) => state.isGameRunning)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const lastLaunchInstanceId = useAppStore((state) => state.lastLaunchInstanceId)
  const activeOrLastInstanceId = launchInstanceId ?? lastLaunchInstanceId
  const pushToast = useAppStore((state) => state.pushToast)

  const [source, setSource] = useState<LogSource>('minecraft')
  const [filter, setFilter] = useState<StreamFilter>('all')
  const [autoScroll, setAutoScroll] = useState(true)
  const [creatingDiag, setCreatingDiag] = useState(false)
  const [diagReport, setDiagReport] = useState<DiagnosticReportResult | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (source !== 'launcher') return
    void invoke('logs:launcherHistory', undefined)
      .then((history) => {
        if (Array.isArray(history)) {
          setLauncherLogs(history)
        }
      })
      .catch(() => {})
  }, [source, setLauncherLogs])

  const activeLines = source === 'minecraft' ? gameLines : launcherLines

  const counts = useMemo(() => {
    let stdout = 0
    let stderr = 0
    let system = 0
    for (const l of activeLines) {
      if (l.stream === 'stdout') stdout++
      else if (l.stream === 'stderr') stderr++
      else if (l.stream === 'system') system++
    }
    return { all: activeLines.length, stdout, stderr, system }
  }, [activeLines])

  const filteredLines = useMemo(() => {
    if (filter === 'all') return activeLines
    return activeLines.filter((entry) => entry.stream === filter)
  }, [activeLines, filter])

  const virtualizer = useVirtualizer({
    count: filteredLines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 25,
    overscan: 24,
  })

  useEffect(() => {
    if (autoScroll && filteredLines.length > 0) {
      virtualizer.scrollToIndex(filteredLines.length - 1, { align: 'end' })
    }
  }, [filteredLines.length, autoScroll, virtualizer, source])

  const handleScroll = (e: UIEvent<HTMLDivElement>): void => {
    const el = e.currentTarget
    const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    const isAtBottom = distanceToBottom <= 50
    if (isAtBottom !== autoScroll) {
      setAutoScroll(isAtBottom)
    }
  }

  const scrollToBottom = (): void => {
    setAutoScroll(true)
    if (filteredLines.length > 0) {
      virtualizer.scrollToIndex(filteredLines.length - 1, { align: 'end' })
    }
  }

  const clearActiveLogs = (): void => {
    if (source === 'minecraft') {
      clearConsole()
    } else {
      clearLauncherLogs()
      void invoke('logs:clearLauncher', undefined).catch(() => {})
    }
  }

  const copyLogs = async (): Promise<void> => {
    if (filteredLines.length === 0) return
    const text = filteredLines
      .map((e) => `[${new Date(e.time).toLocaleTimeString()}] [${e.stream}] ${e.line}`)
      .join('\n')
    try {
      await navigator.clipboard.writeText(text)
      pushToast(t('console.copied'), 'info')
    } catch {
      pushToast(t('error.unknown'), 'error')
    }
  }

  const saveLogs = (): void => {
    if (filteredLines.length === 0) return
    const text = filteredLines
      .map((e) => `[${new Date(e.time).toLocaleTimeString()}] [${e.stream}] ${e.line}`)
      .join('\n')
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    const prefix = source === 'minecraft' ? 'minecraft-console' : 'launcher-logs'
    a.download = `${prefix}-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    setTimeout(() => {
      URL.revokeObjectURL(url)
    }, 500)
    pushToast(t('console.saved'), 'info')
  }

  const openGameLogFolder = (): void => {
    if (!activeOrLastInstanceId) return
    void invoke('instances:openFolder', { id: activeOrLastInstanceId, subfolder: 'logs' }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const openLauncherLogFolder = (): void => {
    void invoke('logs:openLauncherFolder', undefined).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const handleCreateDiagnostic = async (): Promise<void> => {
    setCreatingDiag(true)
    try {
      const report = await invoke('logs:createDiagnostic', {
        instanceId: activeOrLastInstanceId ?? undefined,
        gameLines: gameLines.slice(-300),
        lastError: lastLaunchError?.message ?? undefined,
      })
      setDiagReport(report)
    } catch (error: unknown) {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    } finally {
      setCreatingDiag(false)
    }
  }

  const copyDiagnosticCode = async (): Promise<void> => {
    if (!diagReport) return
    try {
      await navigator.clipboard.writeText(diagReport.code)
      pushToast(t('console.codeCopied'), 'info')
    } catch {
      pushToast(t('error.unknown'), 'error')
    }
  }

  const copyFullReport = async (): Promise<void> => {
    if (!diagReport) return
    try {
      await navigator.clipboard.writeText(diagReport.reportText)
      pushToast(t('console.reportCopied'), 'info')
    } catch {
      pushToast(t('error.unknown'), 'error')
    }
  }

  const stopGame = (): void => {
    if (!launchInstanceId) return
    void invoke('launch:kill', { instanceId: launchInstanceId }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const cancelLaunch = (): void => {
    if (!launchTaskId) return
    void invoke('tasks:cancel', { taskId: launchTaskId }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const statusLabel = isRunning
    ? t('console.statusRunning')
    : launchTaskId
      ? t('console.statusPreparing')
      : t('console.statusIdle')

  return (
    <section className={styles.page} aria-labelledby="console-title" data-testid="console-screen">
      <ScreenHeader
        title={t('console.title')}
        subtitle={t('console.description')}
        titleId="console-title"
        actions={
          <>
            <span
              className={`${styles.status} ${isRunning ? styles.statusRunning : launchTaskId ? styles.statusPreparing : styles.statusIdle}`}
              data-testid="console-status"
            >
              <span className={styles.dot} aria-hidden />
              {statusLabel}
            </span>
            {isRunning ? (
              <GlassButton
                size="sm"
                variant="danger"
                icon={<Square size={13} fill="currentColor" aria-hidden />}
                onClick={stopGame}
                data-testid="console-stop"
              >
                {t('home.stop')}
              </GlassButton>
            ) : launchTaskId ? (
              <GlassButton
                size="sm"
                variant="ghost"
                icon={<X size={14} aria-hidden />}
                onClick={cancelLaunch}
                data-testid="console-cancel"
              >
                {t('home.cancelLaunch')}
              </GlassButton>
            ) : null}
            <GlassButton
              size="sm"
              variant="outline"
              icon={<FileCode2 size={14} aria-hidden />}
              onClick={() => void handleCreateDiagnostic()}
              disabled={creatingDiag}
              data-testid="console-create-diagnostic"
            >
              {creatingDiag ? t('console.creatingCode') : t('console.createCode')}
            </GlassButton>
            <GlassButton
              size="sm"
              icon={<Trash2 size={14} aria-hidden />}
              onClick={clearActiveLogs}
              disabled={activeLines.length === 0}
            >
              {t('console.clear')}
            </GlassButton>
          </>
        }
      />

      {lastLaunchError ? (
        <div className={styles.errorBanner} role="alert" data-testid="console-error-banner">
          <div className={styles.errorBannerBody}>
            <AlertTriangle size={16} className={styles.errorBannerIcon} aria-hidden />
            <div>
              <div className={styles.errorBannerTitle}>{t('console.errorBannerTitle')}</div>
              <div className={styles.errorBannerMessage}>{lastLaunchError.message}</div>
            </div>
          </div>
          <div className={styles.errorBannerActions}>
            <GlassButton
              size="sm"
              variant="primary"
              icon={<FileCode2 size={13} aria-hidden />}
              onClick={() => void handleCreateDiagnostic()}
              disabled={creatingDiag}
            >
              {creatingDiag ? t('console.creatingCode') : t('console.createCode')}
            </GlassButton>
            <button
              type="button"
              className={styles.errorDismiss}
              onClick={() => setLastLaunchError(null)}
              aria-label={t('common.dismiss')}
            >
              <X size={14} aria-hidden />
            </button>
          </div>
        </div>
      ) : null}

      <div className={styles.toolbar}>
        <div className={styles.toolbarFilters}>
          <div className={styles.filterGroup} role="tablist" aria-label="Log source">
            <button
              type="button"
              role="tab"
              aria-selected={source === 'minecraft'}
              className={`${styles.filterChip} ${source === 'minecraft' ? styles.filterChipActive : ''}`}
              onClick={() => {
                setSource('minecraft')
                setAutoScroll(true)
              }}
              data-testid="console-tab-minecraft"
            >
              <span>{t('console.source.minecraft')}</span>
              <span className={styles.badge}>{gameLines.length}</span>
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={source === 'launcher'}
              className={`${styles.filterChip} ${source === 'launcher' ? styles.filterChipActive : ''}`}
              onClick={() => {
                setSource('launcher')
                setAutoScroll(true)
              }}
              data-testid="console-tab-launcher"
            >
              <span>{t('console.source.launcher')}</span>
              <span className={styles.badge}>{launcherLines.length}</span>
            </button>
          </div>

          <div className={styles.filterGroup} role="radiogroup" aria-label="Stream filter">
            {(['all', 'stdout', 'stderr', 'system'] as const).map((streamKey) => {
              const count = counts[streamKey]
              const isActive = filter === streamKey
              const labelKey =
                streamKey === 'all'
                  ? 'console.filterAll'
                  : streamKey === 'stdout'
                    ? 'console.filterStdout'
                    : streamKey === 'stderr'
                      ? 'console.filterStderr'
                      : 'console.filterSystem'
              return (
                <button
                  key={streamKey}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  className={`${styles.filterChip} ${isActive ? styles.filterChipActive : ''}`}
                  onClick={() => setFilter(streamKey)}
                >
                  <span>{t(labelKey)}</span>
                  <span className={styles.badge}>{count}</span>
                </button>
              )
            })}
          </div>
        </div>

        <div className={styles.toolbarActions}>
          <span className={styles.lineCount}>{t('console.lineCount', { count: filteredLines.length })}</span>
          <GlassButton
            size="sm"
            icon={<Copy size={13} aria-hidden />}
            onClick={() => void copyLogs()}
            disabled={filteredLines.length === 0}
            title={t('console.copy')}
          >
            {t('console.copy')}
          </GlassButton>
          <GlassButton
            size="sm"
            icon={<Download size={13} aria-hidden />}
            onClick={saveLogs}
            disabled={filteredLines.length === 0}
            title={t('console.save')}
          >
            {t('console.save')}
          </GlassButton>
          {source === 'launcher' ? (
            <GlassButton
              size="sm"
              icon={<FolderOpen size={13} aria-hidden />}
              onClick={openLauncherLogFolder}
              title={t('console.openLauncherFolder')}
              data-testid="console-open-launcher-logs"
            >
              {t('console.openLauncherFolder')}
            </GlassButton>
          ) : activeOrLastInstanceId ? (
            <GlassButton
              size="sm"
              icon={<FolderOpen size={13} aria-hidden />}
              onClick={openGameLogFolder}
              title={t('console.openFolder')}
              data-testid="console-open-logs"
            >
              {t('console.openFolder')}
            </GlassButton>
          ) : null}
        </div>
      </div>

      <div className={styles.outputWrapper}>
        <div
          ref={scrollRef}
          className={styles.output}
          role="log"
          aria-live="off"
          onScroll={handleScroll}
          data-testid="game-console"
        >
          {filteredLines.length === 0 ? (
            <p className={styles.empty}>
              {source === 'minecraft' ? t('console.empty') : t('console.launcherEmpty')}
            </p>
          ) : (
            <div className={styles.virtualSpace} style={{ height: virtualizer.getTotalSize() }}>
              {virtualizer.getVirtualItems().map((item) => {
                const entry = filteredLines[item.index]
                if (!entry) return null
                return (
                  <div
                    key={`${item.key}-${entry.time}`}
                    className={`${styles.line} ${styles[entry.stream]}`}
                    style={{ transform: `translateY(${item.start}px)` }}
                  >
                    <time dateTime={entry.time}>{new Date(entry.time).toLocaleTimeString()}</time>
                    <span className={styles.stream}>{t(`console.stream.${entry.stream}`)}</span>
                    <span className={styles.text}>{entry.line || ' '}</span>
                  </div>
                )
              })}
            </div>
          )}
        </div>

        {!autoScroll && filteredLines.length > 0 ? (
          <button
            type="button"
            className={styles.scrollBadge}
            onClick={scrollToBottom}
            aria-label={t('console.scrollToBottom')}
          >
            <ArrowDown size={14} aria-hidden />
            <span>{t('console.scrollToBottom')}</span>
          </button>
        ) : null}
      </div>

      <GlassDialog
        open={diagReport !== null}
        onOpenChange={(open) => {
          if (!open) setDiagReport(null)
        }}
        title={t('console.diagnosticTitle')}
        description={t('console.diagnosticDesc')}
        closeLabel={t('common.close')}
        size="md"
        footer={
          <div className={styles.diagFooterActions}>
            <div className={styles.diagFooterGroup}>
              <GlassButton
                size="sm"
                icon={<FolderOpen size={13} aria-hidden />}
                onClick={openLauncherLogFolder}
              >
                {t('console.openLauncherFolder')}
              </GlassButton>
              <GlassButton
                size="sm"
                icon={<Copy size={13} aria-hidden />}
                onClick={() => void copyFullReport()}
              >
                {t('console.copyFullReport')}
              </GlassButton>
            </div>
            <div className={styles.diagFooterGroup}>
              <GlassButton
                size="sm"
                variant="primary"
                icon={<Copy size={13} aria-hidden />}
                onClick={() => void copyDiagnosticCode()}
                data-testid="diagnostic-copy-code"
              >
                {t('console.copyCode')}
              </GlassButton>
            </div>
          </div>
        }
      >
        {diagReport ? (
          <div className={styles.diagStack}>
            <div className={styles.diagMetaGrid}>
              <div className={styles.diagMetaRow}>
                <span className={styles.diagMetaLabel}>{t('console.diagnosticRefId')}</span>
                <span className={styles.diagMetaValue} data-testid="diagnostic-ref-id">
                  {diagReport.refId}
                </span>
              </div>
              <div className={styles.diagMetaRow}>
                <span className={styles.diagMetaLabel}>{t('console.diagnosticFile')}</span>
                <span className={styles.diagMetaValue}>{diagReport.filePath}</span>
              </div>
            </div>

            <div className={styles.diagCodeField}>
              <label htmlFor="diagnostic-code-box" className={styles.diagMetaLabel}>
                {t('console.diagnosticCodeLabel')}
              </label>
              <textarea
                id="diagnostic-code-box"
                className={styles.diagCodeArea}
                readOnly
                value={diagReport.code}
                data-testid="diagnostic-code-box"
              />
            </div>

            <p className={styles.diagSummary}>
              {t('console.diagnosticSummary', {
                launcher: diagReport.launcherLines,
                game: diagReport.gameLines,
              })}
            </p>
          </div>
        ) : null}
      </GlassDialog>
    </section>
  )
}
