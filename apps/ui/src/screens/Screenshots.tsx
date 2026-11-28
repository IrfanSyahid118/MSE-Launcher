import { useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { FolderOpen, Images, RefreshCw } from 'lucide-react'
import type { InstanceScreenshot } from '@mc/shared'
import { useI18n } from '../i18n'
import { errorMessage, invoke } from '../api/launcher'
import { useInstances, useInstanceScreenshots } from '../api/queries'
import { useAppStore } from '../stores/app-store'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { GlassDialog } from '../components/GlassDialog/GlassDialog'
import { GlassSelect } from '../components/GlassSelect/GlassSelect'
import type { GlassSelectOption } from '../components/GlassSelect/GlassSelect'
import { Spinner } from '../components/Spinner/Spinner'
import styles from './Screenshots.module.css'

const ALL_INSTANCES_FILTER = '__all__'

function screenshotSrc(filePath: string): string {
  try {
    return convertFileSrc(filePath)
  } catch {
    return filePath
  }
}

/** Galeri tangkapan layar per instance. */
export function Screenshots(): ReactNode {
  const { t, language } = useI18n()
  const instancesQuery = useInstances()
  const selectedInstanceId = useAppStore((state) => state.selectedInstanceId)
  const pushToast = useAppStore((state) => state.pushToast)

  const [filterInstanceId, setFilterInstanceId] = useState<string>(ALL_INSTANCES_FILTER)
  const [preview, setPreview] = useState<InstanceScreenshot | null>(null)

  const effectiveInstanceId =
    filterInstanceId === ALL_INSTANCES_FILTER ? undefined : filterInstanceId
  const screenshotsQuery = useInstanceScreenshots(effectiveInstanceId)

  const instances = useMemo(() => instancesQuery.data ?? [], [instancesQuery.data])
  const folderTargetInstance =
    instances.find((entry) => entry.id === effectiveInstanceId) ??
    instances.find((entry) => entry.id === selectedInstanceId) ??
    instances[0] ??
    null

  const filterOptions: GlassSelectOption[] = useMemo(() => {
    const opts: GlassSelectOption[] = [{ value: ALL_INSTANCES_FILTER, label: t('screenshots.filterAll') }]
    for (const inst of instances) {
      opts.push({ value: inst.id, label: inst.name })
    }
    return opts
  }, [instances, t])

  const screenshots = screenshotsQuery.data ?? []

  const openFolder = (): void => {
    if (!folderTargetInstance) return
    void invoke('instances:openFolder', {
      id: folderTargetInstance.id,
      subfolder: 'screenshots',
    }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('screenshots.openFolderError')), 'error')
    })
  }

  return (
    <div className={styles.page} data-testid="screenshots-screen">
      <div className={styles.header}>
        <div>
          <h1 className={styles.pageTitle}>{t('screenshots.pageTitle')}</h1>
          <p className={styles.subtitle}>{t('screenshots.subtitle')}</p>
        </div>

        <div className={styles.controls}>
          {instances.length > 0 ? (
            <div className={styles.selectWrap}>
              <GlassSelect
                value={filterInstanceId}
                onValueChange={setFilterInstanceId}
                options={filterOptions}
                testId="screenshots-instance-filter"
              />
            </div>
          ) : null}
          <GlassButton
            size="sm"
            variant="outline"
            icon={screenshotsQuery.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} aria-hidden />}
            disabled={screenshotsQuery.isFetching}
            onClick={() => void screenshotsQuery.refetch()}
          >
            {t('screenshots.refresh')}
          </GlassButton>
          {folderTargetInstance ? (
            <GlassButton
              size="sm"
              variant="outline"
              icon={<FolderOpen size={13} aria-hidden />}
              onClick={openFolder}
            >
              {t('screenshots.openFolder')}
            </GlassButton>
          ) : null}
        </div>
      </div>

      <div className={styles.container}>
        {screenshotsQuery.isLoading ? (
          <div className={styles.empty}>
            <Spinner size={24} />
            <p className={styles.hint}>{t('common.loading')}</p>
          </div>
        ) : screenshots.length === 0 ? (
          <div className={styles.empty}>
            <div className={styles.artwork} aria-hidden>
              <Images size={68} strokeWidth={1.1} />
            </div>
            <h2 className={styles.title}>{t('screenshots.title')}</h2>
            <p className={styles.hint}>{t('screenshots.hint')}</p>
            {folderTargetInstance ? (
              <p className={styles.instanceHint}>{folderTargetInstance.name}</p>
            ) : (
              <p className={styles.instanceHint}>{t('screenshots.createInstanceFirst')}</p>
            )}
          </div>
        ) : (
          <div className={styles.scrollBody}>
            <p className={styles.countMeta}>
              {t('screenshots.count', { count: screenshots.length })}
            </p>
            <div className={styles.grid}>
              {screenshots.map((shot) => (
                <button
                  key={shot.filePath}
                  type="button"
                  className={styles.card}
                  onClick={() => setPreview(shot)}
                >
                  <div className={styles.thumbWrap}>
                    <img
                      src={screenshotSrc(shot.filePath)}
                      alt={shot.fileName}
                      className={styles.thumb}
                      loading="lazy"
                    />
                  </div>
                  <div className={styles.cardMeta}>
                    <span className={styles.cardName} title={shot.fileName}>
                      {shot.fileName}
                    </span>
                    <span className={styles.cardSub}>
                      {shot.instanceName} ·{' '}
                      {new Date(shot.modifiedAt).toLocaleDateString(
                        language === 'id' ? 'id-ID' : 'en-US',
                      )}
                    </span>
                  </div>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      <GlassDialog
        open={preview !== null}
        onOpenChange={(open) => {
          if (!open) setPreview(null)
        }}
        title={preview?.fileName ?? t('screenshots.pageTitle')}
        {...(preview
          ? {
              description: `${preview.instanceName} · ${new Date(preview.modifiedAt).toLocaleString(
                language === 'id' ? 'id-ID' : 'en-US',
              )}`,
            }
          : {})}
        closeLabel={t('common.close')}
        size="lg"
      >
        {preview ? (
          <div className={styles.lightbox}>
            <img
              src={screenshotSrc(preview.filePath)}
              alt={preview.fileName}
              className={styles.lightboxImg}
            />
          </div>
        ) : null}
      </GlassDialog>
    </div>
  )
}
