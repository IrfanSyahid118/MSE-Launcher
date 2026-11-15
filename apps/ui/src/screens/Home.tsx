import { Suspense, lazy, useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { useQueryClient } from '@tanstack/react-query'
import {
  Check,
  ChevronDown,
  ChevronUp,
  Filter,
  MoreHorizontal,
  Play,
  Plus,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Square,
} from 'lucide-react'
import { Popover } from 'radix-ui'
import type { Instance } from '@mc/shared'
import { useI18n } from '../i18n'
import {
  useActiveAccount,
  useInstances,
  useJavaInstallations,
  useSettings,
  useVersions,
} from '../api/queries'
import { useAppStore } from '../stores/app-store'
import { InstanceCard } from '../components/InstanceCard/InstanceCard'
import { InstanceIcon } from '../components/InstanceIcon/InstanceIcon'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { RightSidebar } from '../components/RightSidebar/RightSidebar'
import { Spinner } from '../components/Spinner/Spinner'
import { errorMessage, invoke } from '../api/launcher'
import { registerLaunchTask } from '../api/task-registration'
import styles from './Home.module.css'

const CreateInstanceDialog = lazy(() =>
  import('../components/CreateInstanceDialog/CreateInstanceDialog').then((m) => ({
    default: m.CreateInstanceDialog,
  })),
)

const EMPTY_INSTANCES: Instance[] = []

type SortMode = 'recent' | 'name'
type LoaderFilter = 'all' | 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt'

const SORT_MODES: SortMode[] = ['recent', 'name']
const LOADER_FILTERS: LoaderFilter[] = ['all', 'vanilla', 'fabric', 'forge', 'neoforge', 'quilt']
const LOADER_NAMES: Record<Exclude<LoaderFilter, 'all'>, string> = {
  vanilla: 'Vanilla',
  fabric: 'Fabric',
  forge: 'Forge',
  neoforge: 'NeoForge',
  quilt: 'Quilt',
}

function loaderBadgeLabel(instance: Instance): string {
  if (instance.loader.type === 'vanilla') {
    return 'Vanilla'
  }
  return instance.loader.version
    ? `${instance.loader.type.toUpperCase()} ${instance.loader.version}`
    : instance.loader.type.toUpperCase()
}

export function Home(): ReactNode {
  const { t, language } = useI18n()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const launchInFlightRef = useRef(false)
  const [createDialogOpen, setCreateDialogOpen] = useState(false)
  const versionsQuery = useVersions(createDialogOpen)
  const instancesQuery = useInstances()
  const javaQuery = useJavaInstallations()
  const settingsQuery = useSettings()
  const { account } = useActiveAccount()

  const selectedInstanceId = useAppStore((state) => state.selectedInstanceId)
  const setSelectedInstance = useAppStore((state) => state.setSelectedInstance)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const setLaunchInstanceId = useAppStore((state) => state.setLaunchInstanceId)
  const setLastLaunchInstanceId = useAppStore((state) => state.setLastLaunchInstanceId)
  const pushToast = useAppStore((state) => state.pushToast)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchProgress = useAppStore((state) => state.launchProgress)
  const isGameRunning = useAppStore((state) => state.isGameRunning)
  const setLaunchTaskId = useAppStore((state) => state.setLaunchTaskId)
  const setLaunchProgress = useAppStore((state) => state.setLaunchProgress)

  const [selectorOpen, setSelectorOpen] = useState(false)
  const [jumpInOpen, setJumpInOpen] = useState(true)
  const [searchText, setSearchText] = useState('')
  const [sortMode, setSortMode] = useState<SortMode>('recent')
  const [loaderFilter, setLoaderFilter] = useState<LoaderFilter>('all')
  const [sortOpen, setSortOpen] = useState(false)
  const [filterOpen, setFilterOpen] = useState(false)

  const instances = instancesQuery.data ?? EMPTY_INSTANCES

  const selectedInstance = useMemo(
    () =>
      instances.find((instance) => instance.id === selectedInstanceId) ??
      instances.find((instance) => instance.id === 'default') ??
      instances[0] ??
      null,
    [instances, selectedInstanceId],
  )

  useEffect(() => {
    if (selectedInstance && selectedInstance.id !== selectedInstanceId) {
      setSelectedInstance(selectedInstance.id)
    }
  }, [selectedInstance, selectedInstanceId, setSelectedInstance])

  const isSelectedRunning = Boolean(isGameRunning && launchInstanceId === selectedInstance?.id)
  const isSelectedLaunching = Boolean(
    launchTaskId && launchInstanceId === selectedInstance?.id && !isSelectedRunning,
  )
  const isOtherGameBusy = Boolean((launchTaskId || isGameRunning) && launchInstanceId !== selectedInstance?.id)

  const disabledReason = useMemo(() => {
    if (!account) return t('play.reason.account')
    if (instances.length === 0) return t('home.instances.empty')
    if (!selectedInstance) return t('home.instances.empty')
    if (!selectedInstance.minecraftVersion) return t('home.play.unavailable')
    const preferManaged = settingsQuery.data?.java?.preferManaged ?? true
    const customJava =
      selectedInstance.java?.mode === 'custom' && selectedInstance.java?.path
        ? selectedInstance.java.path
        : settingsQuery.data?.java?.customPath
    if (!preferManaged && !customJava) {
      if (javaQuery.isLoading) return t('play.reason.loading')
      if (!javaQuery.data || javaQuery.data.length === 0) return t('play.reason.unavailable')
    }
    if (isOtherGameBusy) return t('home.otherInstanceBusy')
    return null
  }, [
    account,
    instances.length,
    isOtherGameBusy,
    javaQuery.data,
    javaQuery.isLoading,
    selectedInstance,
    settingsQuery.data,
    t,
  ])

  const libraryInstances = useMemo(() => {
    let result = [...instances]
    if (searchText.trim()) {
      const q = searchText.trim().toLowerCase()
      result = result.filter(
        (inst) =>
          inst.name.toLowerCase().includes(q) ||
          inst.minecraftVersion.toLowerCase().includes(q) ||
          inst.loader.type.toLowerCase().includes(q),
      )
    }
    if (loaderFilter !== 'all') {
      result = result.filter((inst) => inst.loader.type === loaderFilter)
    }
    if (sortMode === 'name') {
      result.sort((a, b) => a.name.localeCompare(b.name))
    } else {
      result.sort((a, b) => {
        const timeA = a.lastPlayedAt ? Date.parse(a.lastPlayedAt) : 0
        const timeB = b.lastPlayedAt ? Date.parse(b.lastPlayedAt) : 0
        if (timeA !== timeB) return timeB - timeA
        return a.name.localeCompare(b.name)
      })
    }
    return result
  }, [instances, loaderFilter, searchText, sortMode])

  const launchInstance = (instance: Instance): void => {
    if (launchInFlightRef.current || launchTaskId !== null || isGameRunning) return
    if (!account) {
      pushToast(t('play.reason.account'), 'error')
      return
    }
    if (!instance.minecraftVersion) {
      pushToast(t('home.play.unavailable'), 'error')
      navigate(`/instances/${instance.id}`)
      return
    }
    launchInFlightRef.current = true
    setSelectedInstance(instance.id)
    setLaunchInstanceId(instance.id)
    setLastLaunchInstanceId(instance.id)
    setLaunchProgress({ stage: t('home.launching'), completed: 0, total: 1000 })

    invoke('launch:start', {
      instanceId: instance.id,
      versionId: instance.minecraftVersion,
    })
      .then((res) => {
        registerLaunchTask(res.taskId, queryClient)
      })
      .catch((error: unknown) => {
        setLaunchTaskId(null)
        setLaunchProgress(null)
        setLaunchInstanceId(null)
        pushToast(errorMessage(error, t('error.unknown')), 'error')
      })
      .finally(() => {
        launchInFlightRef.current = false
      })
  }

  const startGame = (): void => {
    if (!selectedInstance) return
    launchInstance(selectedInstance)
  }

  const stopGame = (): void => {
    const instanceId = launchInstanceId ?? selectedInstance?.id
    if (!instanceId) return
    void invoke('launch:kill', { instanceId }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const cancelLaunch = (): void => {
    if (!launchTaskId) return
    void invoke('tasks:cancel', { taskId: launchTaskId }).catch((error: unknown) => {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    })
  }

  const handleSelectInstance = (instanceId: string): void => {
    setSelectedInstance(instanceId)
    setSelectorOpen(false)
  }

  const lastPlayedText = useMemo(() => {
    if (!selectedInstance?.lastPlayedAt) return t('instances.neverPlayed')
    const d = new Date(selectedInstance.lastPlayedAt)
    return Number.isNaN(d.getTime())
      ? t('instances.neverPlayed')
      : t('instances.lastPlayed', {
          date: new Intl.DateTimeFormat(language, { dateStyle: 'short', timeStyle: 'short' }).format(d),
        })
  }, [language, selectedInstance?.lastPlayedAt, t])

  return (
    <div className={styles.page} data-testid="home-screen">
      <div className={styles.mainGrid}>
        <div className={styles.leftColumn}>
          {/* Section 1: Jump In */}
          <section className={styles.jumpSection} aria-labelledby="jump-in-title">
            <div className={styles.sectionHeaderRow}>
              <button
                type="button"
                className={styles.sectionToggleBtn}
                onClick={() => setJumpInOpen((prev) => !prev)}
                aria-expanded={jumpInOpen}
              >
                <h1 id="jump-in-title" className={styles.jumpInTitle}>
                  {t('home.jumpIn')}
                </h1>
                {jumpInOpen ? (
                  <ChevronUp size={15} aria-hidden />
                ) : (
                  <ChevronDown size={15} aria-hidden />
                )}
              </button>
            </div>

            {jumpInOpen ? (
              <section className={styles.heroCard} aria-label={t('home.instanceSelect')}>
                <div className={styles.heroContent}>
                  <div className={styles.heroIdentity}>
                    {selectedInstance ? (
                      <InstanceIcon instance={selectedInstance} size={42} />
                    ) : null}
                    <div className={styles.heroMeta}>
                      <div className={styles.heroTitleRow}>
                        <h2 className={styles.heroTitle} data-testid="home-title">
                          {selectedInstance ? (
                            <>
                              <span data-testid="summary-instance">{selectedInstance.name}</span>
                              <span className={styles.titleDivider}> · </span>
                              <span data-testid="summary-version">
                                {selectedInstance.minecraftVersion}
                              </span>
                            </>
                          ) : (
                            <span>{t('home.noInstance')}</span>
                          )}
                        </h2>
                        {selectedInstance && Date.now() - new Date(selectedInstance.createdAt).getTime() < 1000 * 60 * 60 * 24 ? (
                          <span className={styles.newInstanceBadge}>
                            <Plus size={10} strokeWidth={3} aria-hidden />
                            <span>{t('home.newInstance')}</span>
                          </span>
                        ) : null}
                      </div>

                      {selectedInstance ? (
                        <div className={styles.heroSubtitle}>
                          <span>
                            {loaderBadgeLabel(selectedInstance)}{' '}
                            {selectedInstance.minecraftVersion}
                          </span>
                          <span className={styles.heroSubtitleDot}>•</span>
                          <span>{lastPlayedText}</span>
                        </div>
                      ) : null}
                    </div>
                  </div>

                  <div className={styles.heroActions}>
                    {isSelectedRunning || isSelectedLaunching ? (
                      <button
                        type="button"
                        className={styles.stopButton}
                        onClick={isSelectedRunning ? stopGame : cancelLaunch}
                        aria-label={isSelectedRunning ? t('home.stop') : t('home.cancelLaunch')}
                        title={isSelectedRunning ? t('home.stopHint') : t('home.cancelHint')}
                      >
                        <Square size={15} fill="currentColor" aria-hidden />
                      </button>
                    ) : null}

                    <button
                      type="button"
                      className={`${styles.playButton} ${isSelectedLaunching ? styles.playButtonInstalling : ''}`}
                      disabled={Boolean(disabledReason) || isSelectedRunning || isSelectedLaunching}
                      onClick={startGame}
                      data-testid="play-button"
                      title={disabledReason || undefined}
                    >
                      {isSelectedLaunching ? (
                        <span>{t('home.launching')}</span>
                      ) : (
                        <>
                          <Play size={14} fill="currentColor" aria-hidden />
                          <span>{isSelectedRunning ? t('home.running') : t('home.play')}</span>
                        </>
                      )}
                    </button>

                    <Popover.Root open={selectorOpen} onOpenChange={setSelectorOpen}>
                      <Popover.Trigger asChild>
                        <button
                          type="button"
                          className={styles.moreButton}
                          aria-label={t('home.instanceSelect')}
                          data-testid="instance-selector-trigger"
                          title={t('home.selectInstanceHint')}
                        >
                          <MoreHorizontal size={18} aria-hidden />
                        </button>
                      </Popover.Trigger>
                      <Popover.Portal>
                        <Popover.Content
                          className={styles.popoverContent}
                          side="bottom"
                          align="end"
                          sideOffset={6}
                        >
                          <div
                            className={styles.popoverPanel}
                            data-testid="instance-selector"
                            role="group"
                            aria-label={t('home.instanceSelect')}
                          >
                            {instances.map((instance) => {
                              const isSelected = instance.id === selectedInstance?.id
                              return (
                                <button
                                  key={instance.id}
                                  type="button"
                                  className={styles.optionItem}
                                  onClick={() => handleSelectInstance(instance.id)}
                                  aria-pressed={isSelected}
                                  data-testid={`instance-option-${instance.id}`}
                                >
                                  <InstanceIcon instance={instance} size={24} />
                                  <div className={styles.optionText}>
                                    <span className={styles.optionName}>{instance.name}</span>
                                    <span className={styles.optionVersion}>
                                      {instance.minecraftVersion} · {loaderBadgeLabel(instance)}
                                    </span>
                                  </div>
                                  {isSelected ? <Check size={14} aria-hidden /> : null}
                                </button>
                              )
                            })}

                            <button
                              type="button"
                              className={styles.manageInstancesBtn}
                              onClick={() => {
                                setSelectorOpen(false)
                                navigate('/instances')
                              }}
                            >
                              <Plus size={14} aria-hidden />
                              <span>{t('home.manageInstances')}</span>
                            </button>
                          </div>
                        </Popover.Content>
                      </Popover.Portal>
                    </Popover.Root>
                  </div>
                </div>

                {isSelectedLaunching && launchProgress ? (
                  <div className={styles.progressStrip}>
                    <div
                      className={styles.progressFill}
                      style={{
                        width: launchProgress.total
                          ? `${Math.min(100, Math.round((launchProgress.completed / launchProgress.total) * 100))}%`
                          : '50%',
                      }}
                    />
                  </div>
                ) : null}
              </section>
            ) : null}
          </section>

          {/* Section 2: Library */}
          <section className={styles.librarySection} aria-labelledby="library-title">
            <div className={styles.libraryHeader}>
              <h2 id="library-title" className={styles.libraryTitle}>
                {t('home.library')}
              </h2>
            </div>

            <div className={styles.libraryToolbar}>
              <div className={styles.toolbarTop}>
                <div className={styles.searchBox}>
                  <Search size={14} className={styles.searchIcon} aria-hidden />
                  <input
                    type="text"
                    className={styles.searchInput}
                    placeholder={t('home.search')}
                    value={searchText}
                    onChange={(e) => setSearchText(e.target.value)}
                    aria-label={t('home.search')}
                    data-testid="home-library-search"
                  />
                </div>

                <button
                  type="button"
                  className={styles.newInstanceBtn}
                  onClick={() => setCreateDialogOpen(true)}
                  data-testid="home-create-instance"
                >
                  <Plus size={14} strokeWidth={2.6} aria-hidden />
                  <span>{t('home.newInstance')}</span>
                </button>
              </div>

              <div className={styles.controlsRow}>
                <div className={styles.controlsLeft}>
                  <Popover.Root open={sortOpen} onOpenChange={setSortOpen}>
                    <Popover.Trigger asChild>
                      <button type="button" className={styles.chipButton} data-testid="home-library-sort">
                        <SlidersHorizontal size={13} aria-hidden />
                        <span>{sortMode === 'recent' ? t('home.sort.recent') : t('home.sort.name')}</span>
                        <ChevronDown size={13} aria-hidden />
                      </button>
                    </Popover.Trigger>
                    <Popover.Portal>
                      <Popover.Content
                        className={styles.popoverContent}
                        side="bottom"
                        align="start"
                        sideOffset={6}
                      >
                        <div className={styles.menuPanel} role="menu" aria-label={t('home.sortMenuAria')}>
                          {SORT_MODES.map((mode) => (
                            <button
                              key={mode}
                              type="button"
                              role="menuitemradio"
                              aria-checked={sortMode === mode}
                              className={styles.menuItem}
                              onClick={() => {
                                setSortMode(mode)
                                setSortOpen(false)
                              }}
                            >
                              <span>{mode === 'recent' ? t('home.sort.recent') : t('home.sort.name')}</span>
                              {sortMode === mode ? <Check size={13} aria-hidden /> : null}
                            </button>
                          ))}
                        </div>
                      </Popover.Content>
                    </Popover.Portal>
                  </Popover.Root>

                  <Popover.Root open={filterOpen} onOpenChange={setFilterOpen}>
                    <Popover.Trigger asChild>
                      <button
                        type="button"
                        className={`${styles.chipButton} ${loaderFilter !== 'all' ? styles.chipButtonActive : ''}`}
                        data-testid="home-library-filter"
                      >
                        <Filter size={13} aria-hidden />
                        <span>{loaderFilter === 'all' ? t('instances.allLoaders') : LOADER_NAMES[loaderFilter]}</span>
                        <ChevronDown size={13} aria-hidden />
                      </button>
                    </Popover.Trigger>
                    <Popover.Portal>
                      <Popover.Content
                        className={styles.popoverContent}
                        side="bottom"
                        align="start"
                        sideOffset={6}
                      >
                        <div className={styles.menuPanel} role="menu" aria-label={t('home.filterMenuAria')}>
                          {LOADER_FILTERS.map((loader) => (
                            <button
                              key={loader}
                              type="button"
                              role="menuitemradio"
                              aria-checked={loaderFilter === loader}
                              className={styles.menuItem}
                              onClick={() => {
                                setLoaderFilter(loader)
                                setFilterOpen(false)
                              }}
                            >
                              <span>{loader === 'all' ? t('instances.allLoaders') : LOADER_NAMES[loader]}</span>
                              {loaderFilter === loader ? <Check size={13} aria-hidden /> : null}
                            </button>
                          ))}
                        </div>
                      </Popover.Content>
                    </Popover.Portal>
                  </Popover.Root>
                </div>
              </div>
            </div>

            {instancesQuery.isLoading ? (
              <div className={styles.libraryGrid} aria-busy="true" aria-label={t('home.loadingInstances')}>
                {Array.from({ length: 4 }, (_, index) => (
                  <div key={index} className={styles.cardSkeleton} />
                ))}
              </div>
            ) : instancesQuery.isError ? (
              <div className={styles.libraryEmpty} role="alert">
                <p className={styles.libraryEmptyTitle}>{t('home.instances.error')}</p>
                <p className={styles.libraryEmptyHint}>{t('common.retry')}</p>
                <GlassButton
                  variant="primary"
                  icon={<RefreshCw size={14} aria-hidden />}
                  onClick={() => {
                    void instancesQuery.refetch()
                  }}
                >
                  {t('common.retry')}
                </GlassButton>
              </div>
            ) : instances.length === 0 ? (
              <div className={styles.libraryEmpty} data-testid="home-library-empty">
                <p className={styles.libraryEmptyTitle}>{t('instances.empty')}</p>
                <p className={styles.libraryEmptyHint}>{t('instances.createFirst')}</p>
                <GlassButton
                  variant="primary"
                  icon={<Plus size={15} aria-hidden />}
                  onClick={() => setCreateDialogOpen(true)}
                >
                  {t('instances.create')}
                </GlassButton>
              </div>
            ) : libraryInstances.length === 0 ? (
              <p className={styles.noResults} data-testid="home-library-noresults">
                {t('common.noResults')}
              </p>
            ) : (
              <div className={styles.libraryGrid} data-testid="home-library-grid">
                {libraryInstances.map((instance) => {
                  const instanceRunning = Boolean(
                    isGameRunning && launchInstanceId === instance.id,
                  )
                  const instanceLaunching = Boolean(
                    launchTaskId && launchInstanceId === instance.id && !instanceRunning,
                  )
                  return (
                    <InstanceCard
                      key={instance.id}
                      instance={instance}
                      onLaunch={launchInstance}
                      isLaunching={instanceLaunching}
                      isRunning={instanceRunning}
                    />
                  )
                })}
              </div>
            )}
          </section>
        </div>

        {/* Kolom kanan: panduan awal dan status akun. */}
        <RightSidebar mode="home" />
      </div>

      <span id="play-disabled-reason" data-testid="play-reason" style={{ display: 'none' }}>
        {disabledReason}
      </span>

      {versionsQuery.isError ? (
        <div className={styles.errorBanner} role="alert">
          <span>{t('home.versions.error')}</span>
          <button
            type="button"
            className={styles.retryVersions}
            onClick={() => {
              void versionsQuery.refetch()
            }}
            disabled={versionsQuery.isFetching}
            aria-label={t('common.retry')}
            title={t('common.retry')}
          >
            {versionsQuery.isFetching ? (
              <Spinner size={13} />
            ) : (
              <RefreshCw size={13} aria-hidden />
            )}
          </button>
        </div>
      ) : null}

      {createDialogOpen ? (
        <Suspense fallback={null}>
          <CreateInstanceDialog open={createDialogOpen} onOpenChange={setCreateDialogOpen} />
        </Suspense>
      ) : null}
    </div>
  )
}
