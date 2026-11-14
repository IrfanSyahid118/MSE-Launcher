import { Suspense, lazy, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { Box, Compass, LayoutGrid, List, Play, Plus, Search, Settings2 } from 'lucide-react'
import type { Instance } from '@mc/shared'
import { useI18n } from '../i18n'
import { useInstances } from '../api/queries'
import { errorMessage } from '../api/launcher'
import { useAppStore } from '../stores/app-store'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { InstanceCard } from '../components/InstanceCard/InstanceCard'
import { InstanceIcon } from '../components/InstanceIcon/InstanceIcon'
import styles from './Instances.module.css'

const CreateInstanceDialog = lazy(() =>
  import('../components/CreateInstanceDialog/CreateInstanceDialog').then((m) => ({
    default: m.CreateInstanceDialog,
  })),
)

export function Instances(): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()
  const instancesQuery = useInstances()
  const [search, setSearch] = useState('')
  const [selectedLoader, setSelectedLoader] = useState<string>('all')
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid')
  const [createOpen, setCreateOpen] = useState(false)
  const pushToast = useAppStore((state) => state.pushToast)
  const setLaunchTaskId = useAppStore((state) => state.setLaunchTaskId)
  const setLaunchInstanceId = useAppStore((state) => state.setLaunchInstanceId)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const isGameRunning = useAppStore((state) => state.isGameRunning)
  const [startingInstanceId, setStartingInstanceId] = useState<string | null>(null)
  const launchInFlightRef = useRef(false)

  const instances = useMemo(() => instancesQuery.data ?? [], [instancesQuery.data])

  const filtered = useMemo(() => {
    return instances.filter((inst) => {
      const matchSearch =
        inst.name.toLowerCase().includes(search.toLowerCase()) ||
        inst.minecraftVersion.toLowerCase().includes(search.toLowerCase())

      const matchLoader =
        selectedLoader === 'all' ||
        inst.loader.type.toLowerCase() === selectedLoader.toLowerCase()

      return matchSearch && matchLoader
    })
  }, [instances, search, selectedLoader])

  const handleLaunch = async (instance: Instance): Promise<void> => {
    if (launchInFlightRef.current || launchTaskId !== null || isGameRunning) {
      return
    }
    launchInFlightRef.current = true
    setStartingInstanceId(instance.id)
    try {
      const res = await window.launcher.invoke('launch:start', {
        instanceId: instance.id,
        versionId: instance.minecraftVersion,
      })
      if (!res.ok) {
        pushToast(res.error.message, 'error')
        return
      }
      setLaunchInstanceId(instance.id)
      setLaunchTaskId(res.data.taskId)
    } catch (err) {
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    } finally {
      launchInFlightRef.current = false
      setStartingInstanceId(null)
    }
  }

  const loaderFilters = ['all', 'vanilla', 'fabric', 'quilt', 'forge', 'neoforge']

  return (
    <div className={styles.page} data-testid="instances-screen">
      {/* Page Header */}
      <div className={styles.headerBar}>
        <div className={styles.titleGroup}>
          <h1 className={styles.title}>{t('instances.title')}</h1>
          <span className={styles.counterBadge}>
            {t('home.instanceCount', { count: instances.length })}
          </span>
        </div>

        <div className={styles.headerActions}>
          <GlassButton
            variant="outline"
            icon={<Compass size={14} aria-hidden />}
            onClick={() => navigate('/browse?tab=modpack')}
            data-testid="browse-modpacks-button"
          >
            {t('browse.modpackAction')}
          </GlassButton>
          <GlassButton
            variant="primary"
            icon={<Plus size={14} aria-hidden />}
            onClick={() => setCreateOpen(true)}
            data-testid="create-instance-button"
          >
            {t('instances.create')}
          </GlassButton>
        </div>
      </div>

      {/* Toolbar: Search, Filters, View Modes */}
      <div className={styles.toolbar}>
        <div className={styles.searchBox}>
          <Search size={14} aria-hidden />
          <input
            type="text"
            className={styles.searchInput}
            aria-label={t('instances.search')}
            placeholder={t('instances.search')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            data-testid="instances-search-input"
          />
        </div>

        <div className={styles.filterChips} role="radiogroup" aria-label={t('instances.filterLabel')}>
          {loaderFilters.map((loader) => (
            <button
              key={loader}
              type="button"
              className={`${styles.chip} ${selectedLoader === loader ? styles.chipActive : ''}`}
              onClick={() => setSelectedLoader(loader)}
              aria-checked={selectedLoader === loader}
              role="radio"
            >
              {loader === 'all'
                ? t('instances.allLoaders')
                : loader.charAt(0).toUpperCase() + loader.slice(1)}
            </button>
          ))}
        </div>

        <div className={styles.toolbarRight}>
          <div className={styles.viewModeToggle} role="group" aria-label={t('instances.viewMode')}>
            <button
              type="button"
              className={`${styles.viewModeBtn} ${viewMode === 'grid' ? styles.viewModeBtnActive : ''}`}
              onClick={() => setViewMode('grid')}
              aria-label={t('instances.viewGrid')}
              aria-pressed={viewMode === 'grid'}
              title={t('instances.viewGrid')}
            >
              <LayoutGrid size={15} aria-hidden />
            </button>
            <button
              type="button"
              className={`${styles.viewModeBtn} ${viewMode === 'list' ? styles.viewModeBtnActive : ''}`}
              onClick={() => setViewMode('list')}
              aria-label={t('instances.viewList')}
              aria-pressed={viewMode === 'list'}
              title={t('instances.viewList')}
            >
              <List size={15} aria-hidden />
            </button>
          </div>
        </div>
      </div>

      {/* Content Rendering: Grid vs List vs Empty */}
      {instancesQuery.isLoading ? (
        <div className={styles.empty}>
          <p className={styles.emptyText}>{t('common.loading')}</p>
        </div>
      ) : filtered.length > 0 ? (
        viewMode === 'grid' ? (
          <div className={styles.grid} data-testid="instances-grid">
            {filtered.map((instance) => (
              <InstanceCard
                key={instance.id}
                instance={instance}
                isLaunching={
                  startingInstanceId === instance.id ||
                  Boolean(launchTaskId && launchInstanceId === instance.id) ||
                  (isGameRunning && launchInstanceId === instance.id)
                }
                onLaunch={handleLaunch}
              />
            ))}
          </div>
        ) : (
          <div className={styles.listView} data-testid="instances-grid">
            {filtered.map((instance) => (
              <div
                key={instance.id}
                className={styles.listRow}
                data-testid={`instance-card-${instance.id}`}
                onClick={() => navigate(`/instances/${instance.id}`)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') navigate(`/instances/${instance.id}`)
                }}
              >
                <div className={styles.listLeft}>
                  <InstanceIcon instance={instance} size={32} />
                  <div className={styles.listTitleGroup}>
                    <h3 className={styles.listName}>{instance.name}</h3>
                    <div className={styles.listMeta}>
                      <span>{instance.minecraftVersion}</span>
                      <span>·</span>
                      <span style={{ textTransform: 'uppercase' }}>{instance.loader.type}</span>
                      <span>·</span>
                      <span>
                        {instance.lastPlayedAt
                          ? t('instances.lastPlayed', {
                              date: new Date(instance.lastPlayedAt).toLocaleDateString(),
                            })
                          : t('instances.neverPlayed')}
                      </span>
                    </div>
                  </div>
                </div>
                <div className={styles.listRight}>
                  {/* First button must open details for e2e compatibility */}
                  <GlassButton
                    size="sm"
                    variant="ghost"
                    icon={<Settings2 size={14} aria-hidden />}
                    onClick={(e) => {
                      e.stopPropagation()
                      navigate(`/instances/${instance.id}`)
                    }}
                    title={t('instances.detail')}
                  >
                    {t('instances.manage')}
                  </GlassButton>
                  <GlassButton
                    size="sm"
                    variant="primary"
                    icon={<Play size={13} aria-hidden />}
                    disabled={startingInstanceId !== null || launchTaskId !== null || isGameRunning}
                    onClick={(e) => {
                      e.stopPropagation()
                      handleLaunch(instance)
                    }}
                  >
                    {t('instances.play')}
                  </GlassButton>
                </div>
              </div>
            ))}
          </div>
        )
      ) : (
        <div className={styles.empty}>
          <Box size={38} color="var(--accent)" aria-hidden />
          <h2 className={styles.emptyText}>
            {search ? t('instances.emptySearch') : t('instances.empty')}
          </h2>
          <p className={styles.emptyHint}>
            {search ? t('instances.emptySearchHint') : t('instances.createFirst')}
          </p>
          <div className={styles.emptyActions}>
            <GlassButton
              variant="primary"
              icon={<Plus size={14} aria-hidden />}
              onClick={() => setCreateOpen(true)}
            >
              {t('instances.create')}
            </GlassButton>
            <GlassButton
              variant="ghost"
              icon={<Compass size={14} aria-hidden />}
              onClick={() => navigate('/browse?tab=modpack')}
            >
              {t('browse.modpackAction')}
            </GlassButton>
          </div>
        </div>
      )}

      {createOpen ? (
        <Suspense fallback={null}>
          <CreateInstanceDialog open={createOpen} onOpenChange={setCreateOpen} />
        </Suspense>
      ) : null}
    </div>
  )
}
