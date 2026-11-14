import { useEffect, useMemo, useRef, useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import {
  AlertCircle,
  Box,
  Check,
  Compass,
  Flame,
  FolderArchive,
  HelpCircle,
  Plus,
  Search,
} from 'lucide-react'
import { compareMinecraftVersions } from '@mc/shared'
import type {
  ContentProjectSummary,
  ContentProviderId,
  ContentType,
  LoaderType,
  ProgressEvent,
} from '@mc/shared'
import { useI18n } from '../../i18n'
import {
  useContentSearch,
  useContentVersions,
  useCreateInstance,
  useInstallModpack,
  useIntegrations,
  useLoaderGames,
  useLoaderVersions,
  useVersions,
} from '../../api/queries'
import { errorMessage } from '../../api/launcher'
import { useAppStore } from '../../stores/app-store'
import { GlassButton } from '../GlassButton/GlassButton'
import { GlassDialog } from '../GlassDialog/GlassDialog'
import { GlassSelect } from '../GlassSelect/GlassSelect'
import { LoaderPicker, loaderAvailability } from '../LoaderPicker/LoaderPicker'
import { ProgressBar } from '../ProgressBar/ProgressBar'
import { Spinner } from '../Spinner/Spinner'
import { VersionBlockIcon } from '../VersionBlockIcon/VersionBlockIcon'
import { VersionChannelFilter } from '../VersionChannelFilter/VersionChannelFilter'
import styles from './CreateInstanceDialog.module.css'

const LOADER_LABELS: Record<'fabric' | 'quilt' | 'forge', string> = {
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
}

const CHECKED_LOADERS = ['fabric', 'quilt', 'forge'] as const
const SEARCH_DEBOUNCE_MS = 300

type InstanceSource = 'vanilla' | 'modrinth' | 'curseforge' | 'import'

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const handler = setTimeout(() => setDebounced(value), delayMs)
    return () => clearTimeout(handler)
  }, [value, delayMs])
  return debounced
}

export interface CreateInstanceDialogProps {
  open: boolean
  onOpenChange(open: boolean): void
}

export function CreateInstanceDialog({ open, onOpenChange }: CreateInstanceDialogProps): ReactNode {
  const { t } = useI18n()
  const pushToast = useAppStore((state) => state.pushToast)
  const upsertInstallJob = useAppStore((state) => state.upsertInstallJob)
  const versionsQuery = useVersions(open)
  const loaderGamesQuery = useLoaderGames(open)
  const integrationsQuery = useIntegrations(open)
  const createInstance = useCreateInstance()
  const installModpack = useInstallModpack()

  const curseForgeReady = integrationsQuery.data?.curseforge.configured ?? false

  const [source, setSource] = useState<InstanceSource>('vanilla')
  const [name, setName] = useState('')
  const [version, setVersion] = useState('')
  const [versionSearch, setVersionSearch] = useState('')
  const [loaderType, setLoaderType] = useState<LoaderType>('vanilla')
  const [loaderVersion, setLoaderVersion] = useState('')

  // Modpack state
  const [modpackSearch, setModpackSearch] = useState('')
  const debouncedModpackSearch = useDebouncedValue(modpackSearch.trim(), SEARCH_DEBOUNCE_MS)
  const [selectedModpack, setSelectedModpack] = useState<ContentProjectSummary | null>(null)
  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(null)
  const [hideRelease, setHideRelease] = useState(false)
  const [hideBeta, setHideBeta] = useState(false)
  const [hideAlpha, setHideAlpha] = useState(false)
  const [installTaskId, setInstallTaskId] = useState<string | null>(null)
  const [installProgress, setInstallProgress] = useState<ProgressEvent | null>(null)

  const wasOpen = useRef(false)

  const releases = useMemo(() => {
    const list = (versionsQuery.data ?? []).filter((v) => v.type === 'release')
    return list.slice().sort((a, b) => {
      const timeA = Date.parse(a.releaseTime)
      const timeB = Date.parse(b.releaseTime)
      if (!Number.isNaN(timeA) && !Number.isNaN(timeB) && timeA !== timeB) {
        return timeB - timeA
      }
      return compareMinecraftVersions(a.id, b.id)
    })
  }, [versionsQuery.data])

  const filteredReleases = useMemo(() => {
    if (!versionSearch.trim()) return releases
    const q = versionSearch.trim().toLowerCase()
    return releases.filter((v) => v.id.toLowerCase().includes(q))
  }, [releases, versionSearch])

  const defaultVersion = releases.find((v) => v.installed)?.id ?? releases[0]?.id ?? ''
  const support = loaderGamesQuery.data
  const supportPending = loaderGamesQuery.isPending

  const isLoaderSupported = (type: LoaderType, minecraftVersion: string): boolean =>
    loaderAvailability(type, minecraftVersion, support, supportPending) === 'available'

  const unavailableLoaders = useMemo(() => {
    if (!support || !version) {
      return []
    }
    return CHECKED_LOADERS.filter((type) => {
      const list = support[type]
      return list !== null && !list.includes(version)
    })
  }, [support, version])

  const supportUnavailable =
    !supportPending &&
    (loaderGamesQuery.isError ||
      (support !== undefined && CHECKED_LOADERS.some((type) => support[type] === null)))
  const loaderSelectable = isLoaderSupported(loaderType, version)

  const loaderVersionsQuery = useLoaderVersions(loaderType, version, loaderSelectable)

  // Modpack search query
  const isModpackSource = source === 'modrinth' || source === 'curseforge'
  const isCurseForgeMissing = source === 'curseforge' && !curseForgeReady

  const modpackQuery = useMemo(() => {
    if (!isModpackSource || isCurseForgeMissing) {
      return null
    }
    return {
      provider: source as ContentProviderId,
      type: 'modpack' as ContentType,
      text: debouncedModpackSearch,
      page: 1,
      pageSize: 30,
    }
  }, [debouncedModpackSearch, isCurseForgeMissing, isModpackSource, source])

  const modpackSearchQuery = useContentSearch(modpackQuery)

  // Modpack versions query
  const modpackVersionsQuery = useContentVersions(
    selectedModpack?.provider ?? 'modrinth',
    selectedModpack?.id ?? '',
    {},
    Boolean(selectedModpack) && isModpackSource,
  )

  const modpackVersions = useMemo(() => {
    const raw = modpackVersionsQuery.data ?? []
    const filtered = raw.filter((v) => {
      if (hideRelease && v.versionType === 'release') return false
      if (hideBeta && v.versionType === 'beta') return false
      if (hideAlpha && v.versionType === 'alpha') return false
      return true
    })
    return filtered.slice().sort((a, b) => {
      const typeRank: Record<string, number> = { release: 3, beta: 2, alpha: 1 }
      const rankDiff = (typeRank[b.versionType] ?? 0) - (typeRank[a.versionType] ?? 0)
      if (rankDiff !== 0) return rankDiff

      const timeA = Date.parse(a.publishedAt)
      const timeB = Date.parse(b.publishedAt)
      if (!Number.isNaN(timeA) && !Number.isNaN(timeB) && timeA !== timeB) {
        return timeB - timeA
      }

      return compareMinecraftVersions(a.versionNumber, b.versionNumber)
    })
  }, [modpackVersionsQuery.data, hideRelease, hideBeta, hideAlpha])

  useEffect(() => {
    if (
      modpackVersions.length > 0 &&
      (!selectedVersionId || !modpackVersions.some((v) => v.id === selectedVersionId))
    ) {
      setSelectedVersionId(modpackVersions[0]?.id ?? null)
    }
  }, [modpackVersions, selectedVersionId])

  // Reset form when dialog opens
  useEffect(() => {
    if (open && !wasOpen.current) {
      setName('')
      setSource('vanilla')
      setVersion(defaultVersion)
      setVersionSearch('')
      setLoaderType('vanilla')
      setLoaderVersion('')
      setModpackSearch('')
      setSelectedModpack(null)
      setSelectedVersionId(null)
      setInstallTaskId(null)
      setInstallProgress(null)
    }
    wasOpen.current = open
  }, [defaultVersion, open])

  useEffect(() => {
    if (open && !version && defaultVersion) {
      setVersion(defaultVersion)
    }
  }, [defaultVersion, open, version])

  useEffect(() => {
    if (!open || loaderType === 'vanilla' || !version) {
      return
    }
    const list = support?.[loaderType as 'fabric' | 'quilt' | 'forge']
    if (list && !list.includes(version)) {
      setLoaderType('vanilla')
      setLoaderVersion('')
    }
  }, [loaderType, open, support, version])

  // Listen to background task for modpack install; progres tampil di dock bawah. Notifikasi ditangani oleh AppShell.
  useEffect(() => {
    if (!installTaskId) return
    const title = selectedModpack?.title ?? name
    upsertInstallJob({ taskId: installTaskId, title, progress: null })
    const offProgress = window.launcher.on('task:progress', ({ taskId: id, progress: value }) => {
      if (id === installTaskId) {
        setInstallProgress(value)
        upsertInstallJob({ taskId: installTaskId, title, progress: value })
      }
    })
    const offDone = window.launcher.on('task:done', ({ taskId: id }) => {
      if (id !== installTaskId) return
      setInstallTaskId(null)
      setInstallProgress(null)
      onOpenChange(false)
    })
    const offError = window.launcher.on('task:error', ({ taskId: id }) => {
      if (id !== installTaskId) return
      setInstallTaskId(null)
      setInstallProgress(null)
    })
    return () => {
      offProgress()
      offDone()
      offError()
    }
  }, [installTaskId, name, onOpenChange, selectedModpack?.title, upsertInstallJob])

  const handleSelectMinecraftVersion = (nextVersion: string): void => {
    setVersion(nextVersion)
    setLoaderVersion('')
    if (
      loaderAvailability(loaderType, nextVersion, support, supportPending) ===
      'unavailable'
    ) {
      setLoaderType('vanilla')
    }
  }

  const handleSubmit = (e: FormEvent): void => {
    e.preventDefault()
    const trimmed = name.trim()

    if (source === 'vanilla') {
      if (!trimmed || !version) return
      createInstance.mutate(
        {
          name: trimmed,
          minecraftVersion: version,
          loader: {
            type: loaderType,
            version: loaderType === 'vanilla' ? null : loaderVersion || null,
          },
        },
        {
          onSuccess: () => {
            pushToast(t('instances.created', { name: trimmed }))
            onOpenChange(false)
          },
          onError: (err) => {
            pushToast(errorMessage(err, t('error.unknown')), 'error')
          },
        },
      )
    } else if (source === 'modrinth' || source === 'curseforge') {
      if (!selectedVersionId) return
      const finalName = trimmed || selectedModpack?.title || 'Modpack'
      installModpack.mutate(
        {
          provider: source,
          versionId: selectedVersionId,
          instanceName: finalName,
        },
        {
          onSuccess: ({ taskId }) => {
            setInstallTaskId(taskId)
            pushToast(t('browse.installing', { title: selectedModpack?.title ?? finalName }))
          },
          onError: (err) => {
            pushToast(errorMessage(err, t('error.unknown')), 'error')
          },
        },
      )
    }
  }

  const isInstalling = Boolean(installTaskId)
  const installPercent =
    installProgress && installProgress.total > 0
      ? Math.min(100, Math.round((installProgress.completed / installProgress.total) * 100))
      : undefined

  const isSubmitDisabled =
    source === 'vanilla'
      ? !name.trim() || !version || !loaderSelectable || createInstance.isPending
      : source === 'modrinth' || source === 'curseforge'
        ? !name.trim() ||
          !selectedModpack ||
          !selectedVersionId ||
          isInstalling ||
          installModpack.isPending
        : true

  return (
    <GlassDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('instances.create')}
      closeLabel={t('common.close')}
      size="xl"
    >
      <form onSubmit={handleSubmit} className={styles.form} data-testid="create-instance-form">
        {/* Source Navigation Stepper */}
        <div className={styles.stepperRow}>
          <div className={styles.sourceTabs} role="tablist">
            <button
              type="button"
              className={`${styles.sourceTab} ${source === 'vanilla' ? styles.sourceTabActive : ''}`}
              role="tab"
              aria-selected={source === 'vanilla'}
              onClick={() => {
                setSource('vanilla')
                setSelectedModpack(null)
              }}
            >
              <Box size={16} aria-hidden />
              <span>Standard Instance</span>
            </button>
            <button
              type="button"
              className={`${styles.sourceTab} ${source === 'modrinth' ? styles.sourceTabActive : ''}`}
              role="tab"
              aria-selected={source === 'modrinth'}
              onClick={() => {
                setSource('modrinth')
                setSelectedModpack(null)
                setSelectedVersionId(null)
              }}
            >
              <Compass size={16} aria-hidden />
              <span>Modrinth Modpack</span>
            </button>
            <button
              type="button"
              className={`${styles.sourceTab} ${source === 'curseforge' ? styles.sourceTabActive : ''}`}
              role="tab"
              aria-selected={source === 'curseforge'}
              onClick={() => {
                setSource('curseforge')
                setSelectedModpack(null)
                setSelectedVersionId(null)
              }}
            >
              <Flame size={16} aria-hidden />
              <span>CurseForge Modpack</span>
            </button>
            <button
              type="button"
              className={`${styles.sourceTab} ${source === 'import' ? styles.sourceTabActive : ''}`}
              role="tab"
              aria-selected={source === 'import'}
              onClick={() => {
                setSource('import')
                setSelectedModpack(null)
              }}
            >
              <FolderArchive size={16} aria-hidden />
              <span>Import Instance</span>
            </button>
          </div>
        </div>

        {/* Instance Identity Card */}
        <div className={styles.identityCard}>
          <div className={styles.iconBox}>
            {selectedModpack?.iconUrl ? (
              <img src={selectedModpack.iconUrl} alt="" className={styles.iconImg} />
            ) : (
              <VersionBlockIcon type="release" size={32} />
            )}
          </div>
          <div className={styles.identityFields}>
            <div className={styles.inputField}>
              <label htmlFor="instance-name-input" className={styles.fieldLabel}>
                {t('instance.name')}
              </label>
              <input
                id="instance-name-input"
                type="text"
                className={styles.input}
                value={name}
                placeholder={t('instance.namePlaceholder')}
                autoFocus
                maxLength={64}
                onChange={(e) => setName(e.target.value)}
                data-testid="instance-name-input"
              />
            </div>
            <div className={styles.inputField} style={{ maxWidth: '200px' }}>
              <span className={styles.fieldLabel}>Instance Group</span>
              <select className={styles.select} disabled>
                <option>Default (Ungrouped)</option>
              </select>
            </div>
          </div>
        </div>

        {/* Two-Column Spacious Wizard Body */}
        <div className={styles.wizardBody}>
          {source === 'vanilla' ? (
            <>
              {/* Column 1: Game Version Selection */}
              <div className={styles.panel}>
                <div className={styles.panelHeader}>
                  <h3 className={styles.panelTitle}>1. Minecraft Version</h3>
                  <span className={styles.panelSubtitle}>
                    {filteredReleases.length} releases available
                  </span>
                </div>

                <div className={styles.searchBox}>
                  <Search size={14} aria-hidden />
                  <input
                    type="text"
                    className={styles.searchInput}
                    value={versionSearch}
                    placeholder="Filter Minecraft versions..."
                    onChange={(e) => setVersionSearch(e.target.value)}
                  />
                </div>

                {/* Main Native Select (for E2E selectOption compatibility) */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                  <label htmlFor="instance-version-select" className={styles.fieldLabel}>
                    Selected Version
                  </label>
                  <select
                    id="instance-version-select"
                    className={styles.select}
                    value={version}
                    onChange={(e) => handleSelectMinecraftVersion(e.target.value)}
                    data-testid="instance-version-select"
                  >
                    {releases.map((v) => (
                      <option key={v.id} value={v.id}>
                        {v.id}
                      </option>
                    ))}
                    {!version ? (
                      <option value="">{t('instance.versionUnavailable')}</option>
                    ) : null}
                  </select>
                </div>

                {/* Rich scrollable version list for great UX */}
                <div className={styles.scrollList}>
                  {(versionSearch.trim() ? filteredReleases : filteredReleases.slice(0, 25)).map((v) => {
                    const isSelected = v.id === version
                    return (
                      <div
                        key={v.id}
                        className={`${styles.versionItem} ${isSelected ? styles.versionItemActive : ''}`}
                        onClick={() => handleSelectMinecraftVersion(v.id)}
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleSelectMinecraftVersion(v.id)
                        }}
                      >
                        <div className={styles.versionItemLeft}>
                          {isSelected ? <Check size={14} color="var(--accent)" /> : <Box size={14} />}
                          <span className={styles.versionName}>Minecraft {v.id}</span>
                          <span className={styles.versionTag}>
                            {v.type === 'release' ? 'Release' : 'Snapshot'}
                          </span>
                        </div>
                        {v.installed ? (
                          <span className={styles.versionInstalledBadge}>Installed</span>
                        ) : null}
                      </div>
                    )
                  })}
                </div>
              </div>

              {/* Column 2: Mod Loader Configuration */}
              <div className={styles.panel}>
                <div className={styles.panelHeader}>
                  <h3 className={styles.panelTitle}>2. Mod Loader & Environment</h3>
                  <span className={styles.panelSubtitle}>Compatibility checked</span>
                </div>

                <div className={styles.configGroup}>
                  <div className={styles.settingField}>
                    <label htmlFor="instance-loader-select" className={styles.settingLabel}>
                      {t('instance.loader')}
                    </label>
                    <LoaderPicker
                      id="instance-loader-select"
                      value={loaderType}
                      minecraftVersion={version}
                      support={support}
                      supportPending={supportPending}
                      onChange={(type) => {
                        setLoaderType(type)
                        setLoaderVersion('')
                      }}
                      testId="instance-loader"
                    />
                    {unavailableLoaders.length > 0 ? (
                      <p className={styles.hint} data-testid="loader-support-hint">
                        {t('instance.loaderUnavailableForVersion', {
                          loaders: unavailableLoaders
                            .map((type) => LOADER_LABELS[type])
                            .join(', '),
                          version,
                        })}
                      </p>
                    ) : null}
                    {supportUnavailable ? (
                      <p className={styles.hint} data-testid="loader-support-unavailable">
                        {t('instance.loaderSupportUnavailable')}
                        <button
                          type="button"
                          className={styles.retry}
                          onClick={() => void loaderGamesQuery.refetch()}
                          disabled={loaderGamesQuery.isFetching}
                        >
                          {loaderGamesQuery.isFetching ? <Spinner size={11} /> : null}
                          {t('instance.loaderSupportRetry')}
                        </button>
                      </p>
                    ) : null}
                  </div>

                  {loaderType !== 'vanilla' ? (
                    <div className={styles.settingField}>
                      <label htmlFor="instance-loader-version-select" className={styles.settingLabel}>
                        {t('instance.loaderVersion')}
                      </label>
                      <select
                        id="instance-loader-version-select"
                        className={styles.select}
                        value={loaderVersion}
                        onChange={(e) => setLoaderVersion(e.target.value)}
                        data-testid="instance-loader-version-select"
                      >
                        <option value="">{t('instance.loaderLatest')}</option>
                        {loaderVersionsQuery.data?.map((loader) => (
                          <option key={loader.version} value={loader.version}>
                            {loader.version}
                          </option>
                        ))}
                      </select>
                      {loaderVersionsQuery.isError ? (
                        <p className={styles.hint}>
                          {t('instance.loaderVersionsUnavailable')}
                        </p>
                      ) : null}
                    </div>
                  ) : null}

                  <div className={styles.settingField} style={{ marginTop: '12px' }}>
                    <span className={styles.settingLabel}>Instance Isolation</span>
                    <p className={styles.hint}>
                      Each instance maintains its own dedicated directory for mods, configurations, and saves.
                    </p>
                  </div>
                </div>
              </div>
            </>
          ) : source === 'modrinth' || source === 'curseforge' ? (
            <>
              {/* Modpack Search & List */}
              <div className={styles.panel}>
                <div className={styles.panelHeader}>
                  <h3 className={styles.panelTitle}>
                    {source === 'modrinth' ? 'Modrinth Catalog' : 'CurseForge Catalog'}
                  </h3>
                </div>

                <div className={styles.searchBox}>
                  <Search size={14} aria-hidden />
                  <input
                    type="text"
                    className={styles.searchInput}
                    value={modpackSearch}
                    placeholder="Search modpacks..."
                    onChange={(e) => setModpackSearch(e.target.value)}
                  />
                </div>

                <div className={styles.scrollList}>
                  {isCurseForgeMissing ? (
                    <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-tertiary)' }}>
                      <AlertCircle size={24} style={{ margin: '0 auto 8px auto' }} />
                      <span>{t('browse.curseforgeMissing')}</span>
                    </div>
                  ) : modpackSearchQuery.isLoading ? (
                    <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-tertiary)' }}>
                      <Spinner size={24} style={{ margin: '0 auto 8px auto' }} />
                      <span>Loading modpacks...</span>
                    </div>
                  ) : modpackSearchQuery.data?.items.length === 0 ? (
                    <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-tertiary)' }}>
                      <span>No modpacks found.</span>
                    </div>
                  ) : (
                    modpackSearchQuery.data?.items.map((pack) => {
                      const isSelected = selectedModpack?.id === pack.id
                      return (
                        <div
                          key={pack.id}
                          className={`${styles.versionItem} ${isSelected ? styles.versionItemActive : ''}`}
                          onClick={() => {
                            setSelectedModpack(pack)
                            setSelectedVersionId(null)
                            if (!name || name === selectedModpack?.title) {
                              setName(pack.title)
                            }
                          }}
                          role="button"
                          tabIndex={0}
                        >
                          <div className={styles.versionItemLeft}>
                            {pack.iconUrl ? (
                              <img src={pack.iconUrl} alt="" style={{ width: 20, height: 20, borderRadius: 4 }} />
                            ) : (
                              <Box size={16} />
                            )}
                            <div>
                              <div style={{ fontWeight: 600, fontSize: '13px' }}>{pack.title}</div>
                              <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
                                {pack.description?.slice(0, 60)}...
                              </div>
                            </div>
                          </div>
                        </div>
                      )
                    })
                  )}
                </div>
              </div>

              {/* Modpack Version & Details */}
              <div className={styles.panel}>
                <div className={styles.panelHeader}>
                  <h3 className={styles.panelTitle}>Modpack Details</h3>
                </div>

                {selectedModpack ? (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '14px', flex: 1 }}>
                    <div style={{ fontSize: '13px', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
                      {selectedModpack.description || 'No description provided.'}
                    </div>

                    <VersionChannelFilter
                      hideRelease={hideRelease}
                      hideBeta={hideBeta}
                      hideAlpha={hideAlpha}
                      onToggleRelease={setHideRelease}
                      onToggleBeta={setHideBeta}
                      onToggleAlpha={setHideAlpha}
                    />

                    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                      <label className={styles.settingLabel}>Select Version</label>
                      <GlassSelect
                        value={selectedVersionId ?? ''}
                        onValueChange={(val) => setSelectedVersionId(val)}
                        options={modpackVersions.map((v) => ({
                          value: v.id,
                          label: v.versionNumber,
                          badge: v.versionType.toUpperCase(),
                        }))}
                        placeholder={t('instance.selectedVersion')}
                        disabled={isInstalling || modpackVersions.length === 0}
                        searchable={modpackVersions.length > 5}
                        searchPlaceholder={t('instance.searchVersions')}
                      />
                    </div>

                    {isInstalling ? (
                      <ProgressBar
                        percent={installPercent}
                        label={
                          installProgress?.stage ||
                          t('browse.installing', { title: selectedModpack.title })
                        }
                      />
                    ) : null}
                  </div>
                ) : (
                  <div style={{ textAlign: 'center', padding: '40px 16px', color: 'var(--text-tertiary)' }}>
                    <span>Select a modpack from the list to configure version and install.</span>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className={styles.dropzone} style={{ gridColumn: 'span 2' }}>
              <FolderArchive size={42} color="var(--accent)" aria-hidden />
              <div className={styles.dropzoneTitle}>{t('instance.sourceImport')}</div>
              <div className={styles.dropzoneHint}>
                Supported formats: Modrinth Pack (.mrpack) and CurseForge Modpack Archive (.zip).
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className={styles.footer}>
          <div className={styles.footerLeft}>
            <GlassButton
              type="button"
              variant="ghost"
              icon={<HelpCircle size={14} aria-hidden />}
              onClick={() =>
                pushToast('Configure Minecraft version and mod loader, then click Create Instance.')
              }
            >
              {t('instance.help')}
            </GlassButton>
          </div>
          <div className={styles.footerActions}>
            <GlassButton type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </GlassButton>
            <GlassButton
              type="submit"
              variant="primary"
              icon={
                isInstalling ? (
                  <Spinner size={14} />
                ) : (
                  <Plus size={14} aria-hidden />
                )
              }
              disabled={isSubmitDisabled}
              data-testid="create-instance-submit"
            >
              {isInstalling
                ? 'Installing...'
                : source === 'vanilla'
                  ? t('instances.create')
                  : 'Install Modpack'}
            </GlassButton>
          </div>
        </div>
      </form>
    </GlassDialog>
  )
}
