import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import {
  AlertCircle,
  ArrowLeft,
  Check,
  Compass,
  PackagePlus,
  Search,
} from 'lucide-react'
import type {
  ContentLoader,
  ContentProjectSummary,
  ContentProviderId,
  ContentType,
  Instance,
  ProgressEvent,
} from '@mc/shared'
import { isVersionCompatibleWithMinecraft } from '@mc/shared'
import { useI18n } from '../../i18n'
import {
  useContentSearch,
  useContentVersions,
  useContentWorlds,
  useInstallContent,
  useIntegrations,
} from '../../api/queries'
import { errorMessage } from '../../api/launcher'
import { useAppStore } from '../../stores/app-store'
import { GlassButton } from '../GlassButton/GlassButton'
import { GlassSelect } from '../GlassSelect/GlassSelect'
import { ProgressBar } from '../ProgressBar/ProgressBar'
import { Spinner } from '../Spinner/Spinner'
import { VersionChannelFilter } from '../VersionChannelFilter/VersionChannelFilter'
import styles from './InstanceDownloader.module.css'

const SEARCH_DEBOUNCE_MS = 300
const PROVIDERS: ContentProviderId[] = ['modrinth', 'curseforge']

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const handler = setTimeout(() => setDebounced(value), delayMs)
    return () => clearTimeout(handler)
  }, [value, delayMs])
  return debounced
}

function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(value)
}

export interface InstanceDownloaderProps {
  instance: Instance
  type: 'mod' | 'shader' | 'resourcepack' | 'datapack' | 'world'
  onBackToManager?: () => void
}

export function InstanceDownloader({
  instance,
  type,
  onBackToManager,
}: InstanceDownloaderProps): ReactNode {
  const { t } = useI18n()
  const pushToast = useAppStore((state) => state.pushToast)
  const upsertInstallJob = useAppStore((state) => state.upsertInstallJob)
  const integrationsQuery = useIntegrations()
  const curseForgeReady = integrationsQuery.data?.curseforge.configured ?? false

  const [provider, setProvider] = useState<ContentProviderId>(
    type === 'world' ? 'curseforge' : 'modrinth',
  )
  const [text, setText] = useState('')
  const debouncedText = useDebouncedValue(text.trim(), SEARCH_DEBOUNCE_MS)
  const [selectedProject, setSelectedProject] = useState<ContentProjectSummary | null>(null)
  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(null)
  const [worldId, setWorldId] = useState('')
  const [hideRelease, setHideRelease] = useState(false)
  const [hideBeta, setHideBeta] = useState(false)
  const [hideAlpha, setHideAlpha] = useState(false)
  const [installTaskId, setInstallTaskId] = useState<string | null>(null)
  const [installProgress, setInstallProgress] = useState<ProgressEvent | null>(null)
  const [failedIconKeys, setFailedIconKeys] = useState<Set<string>>(() => new Set())

  const installMutation = useInstallContent()

  const isDatapack = type === 'datapack'
  const isWorldOnModrinth = type === 'world' && provider === 'modrinth'
  const isCurseForgeMissing = provider === 'curseforge' && !curseForgeReady

  const worldsQuery = useContentWorlds(instance.id, isDatapack)
  const worlds = worldsQuery.data ?? []

  useEffect(() => {
    setWorldId('')
  }, [instance.id, type])

  const targetLoader = useMemo<ContentLoader | undefined>(() => {
    if (type === 'mod') {
      return instance.loader.type !== 'vanilla' ? instance.loader.type : undefined
    }
    if (type === 'datapack') {
      return 'datapack'
    }
    return undefined
  }, [type, instance.loader.type])

  const query = useMemo(() => {
    if (isWorldOnModrinth || isCurseForgeMissing) {
      return null
    }
    return {
      provider,
      type: type as ContentType,
      text: debouncedText,
      page: 1,
      pageSize: 30,
      minecraftVersion: instance.minecraftVersion,
      ...(targetLoader ? { loader: targetLoader } : {}),
    }
  }, [
    isWorldOnModrinth,
    isCurseForgeMissing,
    provider,
    type,
    debouncedText,
    instance.minecraftVersion,
    targetLoader,
  ])

  const searchQuery = useContentSearch(query)
  const items = useMemo(() => searchQuery.data?.items ?? [], [searchQuery.data?.items])

  // Auto-select first item when items change
  useEffect(() => {
    if (items.length > 0) {
      if (!selectedProject || !items.some((it) => it.id === selectedProject.id)) {
        const first = items[0]
        if (first) setSelectedProject(first)
      }
    } else {
      setSelectedProject(null)
    }
  }, [items, selectedProject])

  // Query versions for the selected project
  const versionsFilter = useMemo(
    () => ({
      minecraftVersion: instance.minecraftVersion,
      ...(targetLoader ? { loader: targetLoader } : {}),
    }),
    [instance.minecraftVersion, targetLoader],
  )

  const versionsQuery = useContentVersions(
    selectedProject?.provider ?? 'modrinth',
    selectedProject?.id ?? '',
    versionsFilter,
    Boolean(selectedProject),
  )

  const compatibleVersions = useMemo(() => {
    const raw = versionsQuery.data ?? []
    const filtered = raw.filter((v) => {
      if (hideRelease && v.versionType === 'release') return false
      if (hideBeta && v.versionType === 'beta') return false
      if (hideAlpha && v.versionType === 'alpha') return false

      if (
        !isVersionCompatibleWithMinecraft(
          v.gameVersions,
          instance.minecraftVersion,
          v.versionNumber,
          v.name,
        )
      ) {
        return false
      }

      if (type === 'datapack') {
        if (v.loaders.length > 0 && !v.loaders.includes('datapack')) {
          return false
        }
      } else if (type === 'mod' && instance.loader.type !== 'vanilla') {
        if (v.loaders.length > 0 && !v.loaders.includes(instance.loader.type)) {
          return false
        }
      }

      return true
    })
    return filtered.slice().sort((a, b) => {
      const typeRank: Record<string, number> = { release: 3, beta: 2, alpha: 1 }
      const rankDiff = (typeRank[b.versionType] ?? 0) - (typeRank[a.versionType] ?? 0)
      if (rankDiff !== 0) return rankDiff
      return Date.parse(b.publishedAt) - Date.parse(a.publishedAt)
    })
  }, [
    versionsQuery.data,
    hideRelease,
    hideBeta,
    hideAlpha,
    instance.minecraftVersion,
    instance.loader.type,
    type,
  ])

  // Select latest version by default
  useEffect(() => {
    if (compatibleVersions.length > 0) {
      setSelectedVersionId((curr) => {
        if (curr && compatibleVersions.some((v) => v.id === curr)) {
          return curr
        }
        return compatibleVersions[0]?.id ?? null
      })
    } else {
      setSelectedVersionId(null)
    }
  }, [compatibleVersions])

  // Task event listeners; progres tampil di dock bawah. Notifikasi selesai ditangani terpusat oleh AppShell.
  useEffect(() => {
    if (!installTaskId) return
    const title = selectedProject?.title ?? t('browse.installing')
    upsertInstallJob({ taskId: installTaskId, title, progress: null })
    const offProg = window.launcher.on('task:progress', ({ taskId, progress }) => {
      if (taskId === installTaskId) {
        setInstallProgress(progress)
        upsertInstallJob({ taskId: installTaskId, title, progress })
      }
    })
    const offDone = window.launcher.on('task:done', ({ taskId }) => {
      if (taskId === installTaskId) {
        setInstallTaskId(null)
        setInstallProgress(null)
      }
    })
    const offErr = window.launcher.on('task:error', ({ taskId }) => {
      if (taskId === installTaskId) {
        setInstallTaskId(null)
        setInstallProgress(null)
      }
    })
    return () => {
      offProg()
      offDone()
      offErr()
    }
  }, [installTaskId, selectedProject?.title, t, upsertInstallJob])

  const installedProjectIds = useMemo(() => {
    return new Set((instance.content ?? []).map((c) => c.projectId))
  }, [instance.content])

  const isSelectedInstalled = selectedProject ? installedProjectIds.has(selectedProject.id) : false

  const iconKey = (item: ContentProjectSummary): string => `${item.provider}:${item.id}`
  const markIconFailed = (item: ContentProjectSummary): void => {
    const key = iconKey(item)
    setFailedIconKeys((current) => {
      if (current.has(key)) return current
      return new Set(current).add(key)
    })
  }

  const handleInstall = async (): Promise<void> => {
    if (!selectedProject || !selectedVersionId || (isDatapack && !worldId)) return
    try {
      const res = await installMutation.mutateAsync({
        instanceId: instance.id,
        provider: selectedProject.provider,
        versionId: selectedVersionId,
        ...(isDatapack ? { worldId } : {}),
      })
      setInstallTaskId(res.taskId)
    } catch (err) {
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    }
  }

  const percent =
    installProgress && installProgress.total > 0
      ? Math.min(100, Math.round((installProgress.completed / installProgress.total) * 100))
      : undefined

  return (
    <div className={styles.container} data-testid="instance-downloader">
      {/* Top Controls Bar */}
      <div className={styles.topBar}>
        <div className={styles.leftControls}>
          {onBackToManager ? (
            <GlassButton
              variant="ghost"
              size="sm"
              icon={<ArrowLeft size={13} aria-hidden />}
              onClick={onBackToManager}
            >
              {t('instance.viewManager')}
            </GlassButton>
          ) : null}

          <div className={styles.searchBox}>
            <Search size={14} aria-hidden />
            <input
              type="text"
              className={styles.searchInput}
              placeholder={t('instance.downloader.searchSimple', {
                type:
                  type === 'mod'
                    ? t('browse.tab.mods')
                    : type === 'shader'
                      ? t('browse.tab.shaders')
                      : type === 'resourcepack'
                        ? t('browse.tab.resourcepacks')
                        : type === 'datapack'
                          ? t('browse.tab.datapacks')
                          : t('browse.tab.worlds'),
              })}
              value={text}
              onChange={(e) => setText(e.target.value)}
              data-testid="instance-downloader-search"
            />
          </div>

          <div className={styles.versionBadges}>
            <span className={styles.versionChip} title={t('instance.version')}>
              Minecraft {instance.minecraftVersion}
            </span>
            {type === 'mod' && instance.loader.type !== 'vanilla' ? (
              <span className={styles.loaderChip}>{instance.loader.type}</span>
            ) : null}
          </div>
        </div>

        <div className={styles.rightControls}>
          <span className={styles.providerLabel}>{t('browse.provider')}</span>
          <div className={styles.segmented} role="group" aria-label={t('browse.provider')}>
            {PROVIDERS.map((value) => {
              const isSelected = provider === value
              return (
                <button
                  key={value}
                  type="button"
                  aria-pressed={isSelected}
                  className={styles.segment}
                  onClick={() => setProvider(value)}
                  data-testid={`instance-downloader-provider-${value}`}
                >
                  <span className={styles.segmentLabel}>{t(`browse.provider.${value}`)}</span>
                </button>
              )
            })}
          </div>
        </div>
      </div>

      {/* Split Master-Detail Layout */}
      {isWorldOnModrinth ? (
        <div className={styles.state}>
          <Compass size={24} className={styles.stateIcon} aria-hidden />
          <p className={styles.stateText}>{t('browse.worldNeedsCurseforge')}</p>
          <p className={styles.stateHint}>{t('browse.worldNeedsCurseforgeHint')}</p>
          <GlassButton variant="outline" size="sm" onClick={() => setProvider('curseforge')}>
            {t('browse.useCurseforge')}
          </GlassButton>
        </div>
      ) : isCurseForgeMissing ? (
        <div className={styles.state}>
          <AlertCircle size={24} className={styles.stateIcon} aria-hidden />
          <p className={styles.stateText}>{t('browse.curseforgeMissing')}</p>
          <p className={styles.stateHint}>{t('browse.curseforgeMissingHint')}</p>
        </div>
      ) : searchQuery.isLoading ? (
        <div className={styles.state}>
          <Spinner size={24} />
          <p className={styles.stateText}>{t('browse.searching')}</p>
        </div>
      ) : searchQuery.isError ? (
        <div className={styles.state} role="alert">
          <p className={styles.stateText}>
            {errorMessage(searchQuery.error, t('browse.searchFailed'))}
          </p>
          <GlassButton variant="outline" size="sm" onClick={() => void searchQuery.refetch()}>
            {t('common.retry')}
          </GlassButton>
        </div>
      ) : items.length === 0 ? (
        <div className={styles.state}>
          <p className={styles.stateText}>
            {debouncedText
              ? t('browse.emptyForQuery', { query: debouncedText })
              : t('instance.downloader.noResults', {
                  type: t(`instance.contentType.${type}`),
                  version: instance.minecraftVersion,
                })}
          </p>
          <p className={styles.stateHint}>{t('browse.emptyHint')}</p>
        </div>
      ) : (
        <div className={styles.splitView}>
          {/* Master List Column */}
          <div className={styles.masterPane}>
            <div className={styles.masterHeader}>
              <span>
                {type === 'mod'
                  ? t('browse.tab.mods')
                  : type === 'shader'
                    ? t('browse.tab.shaders')
                    : type === 'resourcepack'
                      ? t('browse.tab.resourcepacks')
                      : type === 'datapack'
                        ? t('browse.tab.datapacks')
                        : t('browse.tab.worlds')}{' '}
                ({items.length})
              </span>
              <span style={{ textTransform: 'capitalize' }}>{provider}</span>
            </div>
            <div className={styles.itemList} role="listbox">
              {items.map((item) => {
                const isSelected = selectedProject?.id === item.id
                const isInstalled = installedProjectIds.has(item.id)
                return (
                  <button
                    key={item.id}
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    className={`${styles.itemRow} ${isSelected ? styles.itemRowActive : ''}`}
                    onClick={() => setSelectedProject(item)}
                  >
                    {item.iconUrl && !failedIconKeys.has(iconKey(item)) ? (
                      <img
                        src={item.iconUrl}
                        alt=""
                        className={styles.itemIcon}
                        loading="lazy"
                        onError={() => markIconFailed(item)}
                      />
                    ) : (
                      <div className={styles.itemIconFallback}>
                        {item.title.charAt(0).toUpperCase()}
                      </div>
                    )}
                    <div className={styles.itemDetails}>
                      <div className={styles.itemTitleRow}>
                        <span className={styles.itemTitle}>{item.title}</span>
                        {isInstalled ? (
                          <span className={styles.installedTag}>
                            <Check size={10} aria-hidden /> {t('instance.downloader.installed')}
                          </span>
                        ) : null}
                      </div>
                      <p className={styles.itemDescription}>{item.description}</p>
                      <div className={styles.itemMeta}>
                        <span>{item.slug}</span>
                        <span>•</span>
                        <span>{formatCount(item.downloads)} unduhan</span>
                      </div>
                    </div>
                  </button>
                )
              })}
            </div>
          </div>

          {/* Detail Pane Column */}
          <div className={styles.detailPane}>
            {selectedProject ? (
              <>
                <div className={styles.detailHeader}>
                  {selectedProject.iconUrl && !failedIconKeys.has(iconKey(selectedProject)) ? (
                    <img
                      src={selectedProject.iconUrl}
                      alt=""
                      className={styles.detailIcon}
                      onError={() => markIconFailed(selectedProject)}
                    />
                  ) : (
                    <div className={styles.detailIconFallback}>
                      {selectedProject.title.charAt(0).toUpperCase()}
                    </div>
                  )}
                  <div className={styles.detailTitleArea}>
                    <h3 className={styles.detailTitle}>{selectedProject.title}</h3>
                    <p className={styles.detailAuthor}>{selectedProject.slug}</p>
                  </div>
                </div>

                <div className={styles.detailDescription}>
                  {selectedProject.description}
                </div>

                {/* Version Selector Dropdown */}
                <div className={styles.versionField}>
                  <label htmlFor="downloader-version-select" className={styles.versionLabel}>
                    {t('browse.version')}
                  </label>
                  <VersionChannelFilter
                    hideRelease={hideRelease}
                    hideBeta={hideBeta}
                    hideAlpha={hideAlpha}
                    onToggleRelease={setHideRelease}
                    onToggleBeta={setHideBeta}
                    onToggleAlpha={setHideAlpha}
                  />
                  {versionsQuery.isLoading ? (
                    <div className={styles.noVersionsHint}>
                      <Spinner size={13} /> Memuat versi cocok…
                    </div>
                  ) : compatibleVersions.length > 0 ? (
                    <GlassSelect
                      id="downloader-version-select"
                      value={selectedVersionId ?? ''}
                      onValueChange={(val) => setSelectedVersionId(val)}
                      options={compatibleVersions.map((v) => ({
                        value: v.id,
                        label: v.versionNumber,
                        badge: v.versionType.toUpperCase(),
                      }))}
                      placeholder={t('browse.version')}
                      disabled={installTaskId !== null}
                      searchable={compatibleVersions.length > 5}
                      searchPlaceholder={t('instance.searchVersions')}
                    />
                  ) : (
                    <p className={styles.noVersionsHint}>
                      {t('browse.noCompatibleVersions')}
                    </p>
                  )}
                </div>

                {isDatapack ? (
                  <div className={styles.versionField}>
                    <label htmlFor="downloader-world-select" className={styles.versionLabel}>
                      {t('browse.world')}
                    </label>
                    {worldsQuery.isLoading ? (
                      <div className={styles.noVersionsHint}>
                        <Spinner size={13} /> {t('common.loading')}
                      </div>
                    ) : worlds.length === 0 ? (
                      <p className={styles.noVersionsHint}>{t('browse.noWorlds')}</p>
                    ) : (
                      <select
                        id="downloader-world-select"
                        className={styles.versionSelect}
                        value={worldId}
                        disabled={installTaskId !== null}
                        onChange={(e) => setWorldId(e.target.value)}
                        data-testid="instance-downloader-world-select"
                      >
                        <option value="">{t('browse.worldPlaceholder')}</option>
                        {worlds.map((world) => (
                          <option key={world.folder} value={world.folder}>
                            {world.name}
                          </option>
                        ))}
                      </select>
                    )}
                  </div>
                ) : null}

                {/* Progress / Install Action */}
                <div className={styles.detailActionArea}>
                  {installTaskId !== null && installProgress ? (
                    <ProgressBar
                      percent={percent}
                      label={installProgress.stage || t('browse.installing')}
                    />
                  ) : (
                    <GlassButton
                      variant={isSelectedInstalled ? 'outline' : 'primary'}
                      icon={
                        installMutation.isPending ? (
                          <Spinner size={14} />
                        ) : isSelectedInstalled ? (
                          <Check size={14} aria-hidden />
                        ) : (
                          <PackagePlus size={14} aria-hidden />
                        )
                      }
                      disabled={
                        !selectedVersionId ||
                        compatibleVersions.length === 0 ||
                        (isDatapack && !worldId) ||
                        installMutation.isPending ||
                        installTaskId !== null
                      }
                      onClick={() => void handleInstall()}
                    >
                      {installMutation.isPending
                        ? t('browse.installing')
                        : isSelectedInstalled
                          ? t('instance.downloader.installed') + ' (Pasang Ulang)'
                          : t('instance.downloader.install')}
                    </GlassButton>
                  )}
                </div>
              </>
            ) : (
              <div className={styles.emptySelection}>
                <PackagePlus size={24} aria-hidden />
                <span>Pilih salah satu item di daftar untuk melihat rincian & versi.</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
