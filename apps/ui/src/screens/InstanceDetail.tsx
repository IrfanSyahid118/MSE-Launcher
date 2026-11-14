import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate, useParams } from 'react-router'
import { DropdownMenu } from 'radix-ui'
import { useVirtualizer } from '@tanstack/react-virtual'
import {
  ArrowLeft,
  Check,
  Copy,
  Download,
  FileCode,
  Filter,
  FolderOpen,
  Globe,
  Link2,
  MoreVertical,
  Palette,
  Pencil,
  Play,
  Plus,
  Puzzle,
  RefreshCw,
  Save,
  Search,
  Server,
  Settings as SettingsIcon,
  SlidersHorizontal,
  Sun,
  Trash2,
  X,
} from 'lucide-react'
import type {
  ContentProviderId,
  ContentType,
  InstanceContentEntry,
  InstanceServerEntry,
  LoaderType,
  ServerPingStatus,
  ServerResourcePackPolicy,
} from '@mc/shared'
import { compareMinecraftVersions, supportsDatapacks } from '@mc/shared'
import { useI18n } from '../i18n'
import type { MessageKey } from '../i18n'
import {
  useAddInstanceServer,
  useContentUpdates,
  useContentWorlds,
  useEditInstanceServer,
  useInstanceServers,
  useInstances,
  useRemoveContent,
  useRemoveInstance,
  useRemoveInstanceServer,
  useToggleContent,
  useUpdateInstance,
  useVersions,
} from '../api/queries'
import { errorMessage, invoke } from '../api/launcher'
import { useAppStore } from '../stores/app-store'
import { GlassButton } from '../components/GlassButton/GlassButton'
import { GlassCard } from '../components/GlassCard/GlassCard'
import { GlassDialog } from '../components/GlassDialog/GlassDialog'
import { GlassSelect } from '../components/GlassSelect/GlassSelect'
import type { GlassSelectOption } from '../components/GlassSelect/GlassSelect'
import { GlassSwitch } from '../components/GlassSwitch/GlassSwitch'
import { InstanceDownloader } from '../components/InstanceDownloader/InstanceDownloader'
import { InstanceIcon } from '../components/InstanceIcon/InstanceIcon'

import styles from './InstanceDetail.module.css'
import { Spinner } from '../components/Spinner/Spinner'

const CONTENT_TYPE_KEYS: Record<ContentType, MessageKey> = {
  mod: 'instance.contentType.mod',
  shader: 'instance.contentType.shader',
  resourcepack: 'instance.contentType.resourcepack',
  datapack: 'instance.contentType.datapack',
  world: 'instance.contentType.world',
  modpack: 'instance.contentType.modpack',
}

type InstanceManageTab =
  | 'overview'
  | 'mods'
  | 'resourcepacks'
  | 'shaders'
  | 'datapacks'
  | 'worlds'
  | 'servers'
  | 'settings'

interface VirtualizedContentListProps {
  items: InstanceContentEntry[]
  updatesMap: Map<string, { latestVersionNumber: string }>
  icon: ReactNode
  scrollContainerRef: React.RefObject<HTMLDivElement | null>
  isGameRunning: boolean
  togglePending: boolean
  removePending: boolean
  onToggle: (provider: ContentProviderId, id: string, active: boolean, fileName: string) => void
  onRemove: (provider: ContentProviderId, id: string) => void
  t: (key: MessageKey, params?: Record<string, string | number>) => string
}

function VirtualizedContentList({
  items,
  updatesMap,
  icon,
  scrollContainerRef,
  isGameRunning,
  togglePending,
  removePending,
  onToggle,
  onRemove,
  t,
}: VirtualizedContentListProps): ReactNode {
  const containerRef = useRef<HTMLDivElement>(null)
  const [offsetTop, setOffsetTop] = useState(0)

  useEffect(() => {
    if (containerRef.current && scrollContainerRef.current) {
      const cRect = containerRef.current.getBoundingClientRect()
      const sRect = scrollContainerRef.current.getBoundingClientRect()
      setOffsetTop(cRect.top - sRect.top + scrollContainerRef.current.scrollTop)
    }
  }, [scrollContainerRef])

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => 64,
    overscan: 6,
    scrollMargin: offsetTop,
    gap: 8,
  })

  return (
    <div
      ref={containerRef}
      style={{
        height: `${virtualizer.getTotalSize()}px`,
        width: '100%',
        position: 'relative',
      }}
    >
      {virtualizer.getVirtualItems().map((virtualRow) => {
        const entry = items[virtualRow.index]
        if (!entry) return null
        const update = updatesMap.get(`${entry.provider}:${entry.projectId}`)
        return (
          <div
            key={`${entry.provider}:${entry.projectId}`}
            ref={virtualizer.measureElement}
            data-index={virtualRow.index}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              transform: `translateY(${virtualRow.start - offsetTop}px)`,
            }}
          >
            <div
              className={`${styles.contentRow} ${entry.disabled ? styles.contentRowDisabled : ''}`}
            >
              <div className={styles.itemDetails}>
                <div className={`${styles.itemIcon} ${entry.disabled ? styles.itemIconDisabled : ''}`}>
                  {icon}
                </div>
                <div className={styles.itemText}>
                  <div className={styles.contentTitleRow}>
                    <span
                      className={`${styles.contentName} ${entry.disabled ? styles.contentNameDisabled : ''}`}
                      title={entry.fileName}
                    >
                      {entry.fileName}
                    </span>
                    <span className={entry.disabled ? styles.statusBadgeDisabled : styles.statusBadgeActive}>
                      {entry.disabled ? t('instance.modDisabled') : t('instance.modEnabled')}
                    </span>
                    {update ? (
                      <span
                        className={styles.updateBadge}
                        title={t('instance.updateAvailable', {
                          version: update.latestVersionNumber,
                        })}
                      >
                        {t('instance.updateBadge', {
                          version: update.latestVersionNumber,
                        })}
                      </span>
                    ) : null}
                  </div>
                  <span className={styles.contentMeta}>
                    {t(CONTENT_TYPE_KEYS[entry.type])} · {entry.provider}
                  </span>
                </div>
              </div>
              <div className={styles.itemActions}>
                <div className={styles.toggleWrap}>
                  <GlassSwitch
                    checked={!entry.disabled}
                    onCheckedChange={(checked) =>
                      onToggle(entry.provider, entry.projectId, checked, entry.fileName)
                    }
                    label={entry.disabled ? t('instance.enableMod') : t('instance.disableMod')}
                    disabled={togglePending || isGameRunning}
                  />
                  <span
                    className={`${styles.toggleLabel} ${entry.disabled ? styles.toggleLabelDisabled : styles.toggleLabelActive}`}
                  >
                    {entry.disabled ? t('instance.modDisabled') : t('instance.modEnabled')}
                  </span>
                </div>
                <GlassButton
                  variant="ghost"
                  size="sm"
                  icon={<Trash2 size={13} aria-hidden />}
                  disabled={removePending || isGameRunning}
                  onClick={() => onRemove(entry.provider, entry.projectId)}
                  aria-label={t('instance.contentRemove')}
                  title={t('instance.contentRemove')}
                >
                  {t('instance.contentRemove')}
                </GlassButton>
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}

export function InstanceDetail(): ReactNode {
  const { id } = useParams<{ id: string }>()
  const { t, language } = useI18n()
  const navigate = useNavigate()
  const instancesQuery = useInstances()
  const removeInstance = useRemoveInstance()
  const updateInstance = useUpdateInstance()
  const removeContent = useRemoveContent()
  const toggleContent = useToggleContent()
  const versionsQuery = useVersions()
  const pushToast = useAppStore((state) => state.pushToast)
  const setLaunchTaskId = useAppStore((state) => state.setLaunchTaskId)
  const setLaunchInstanceId = useAppStore((state) => state.setLaunchInstanceId)
  const setLastLaunchInstanceId = useAppStore((state) => state.setLastLaunchInstanceId)
  const setGlobalCleanupTaskId = useAppStore((state) => state.setCleanupTaskId)
  const launchTaskId = useAppStore((state) => state.launchTaskId)
  const launchInstanceId = useAppStore((state) => state.launchInstanceId)
  const isGameRunning = useAppStore((state) => state.isGameRunning)
  const pageRef = useRef<HTMLDivElement>(null)

  const [activeTab, setActiveTab] = useState<InstanceManageTab>('overview')
  const [viewMode, setViewMode] = useState<'manager' | 'downloader'>('manager')
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleteGameData, setDeleteGameData] = useState(true)
  const [searchFilter, setSearchFilter] = useState('')

  const [editName, setEditName] = useState('')
  const [editVersion, setEditVersion] = useState('')
  const [editLoaderType, setEditLoaderType] = useState<LoaderType>('vanilla')
  const [editLoaderVersion, setEditLoaderVersion] = useState<string | null>(null)

  const serversQuery = useInstanceServers(id ?? '')
  const addServer = useAddInstanceServer()
  const removeServer = useRemoveInstanceServer()
  const editServer = useEditInstanceServer()

  const [serverDialogOpen, setServerDialogOpen] = useState(false)
  const [serverDialogMode, setServerDialogMode] = useState<'add' | 'edit'>('add')
  const [serverNameInput, setServerNameInput] = useState('')
  const [serverIpInput, setServerIpInput] = useState('')
  const [serverResourcePackInput, setServerResourcePackInput] =
    useState<ServerResourcePackPolicy>('prompt')
  const [serverHideFromHomeInput, setServerHideFromHomeInput] = useState(false)
  const [editingOldIp, setEditingOldIp] = useState('')
  const [serverDeleteTarget, setServerDeleteTarget] = useState<InstanceServerEntry | null>(null)
  const [copiedIp, setCopiedIp] = useState<string | null>(null)
  const [serverStatusFilter, setServerStatusFilter] = useState<'all' | 'online'>('all')
  const [serverStatuses, setServerStatuses] = useState<
    Record<string, ServerPingStatus | 'loading'>
  >({})
  const [isRefreshingServers, setIsRefreshingServers] = useState(false)
  const autoPingedIpsRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    setViewMode('manager')
    setSearchFilter('')
  }, [activeTab])

  const instance = instancesQuery.data?.find((inst) => inst.id === id)
  const isVanilla = instance?.loader.type === 'vanilla'
  const isDatapackSupported = useMemo(
    () => (instance?.minecraftVersion ? supportsDatapacks(instance.minecraftVersion) : false),
    [instance?.minecraftVersion],
  )

  useEffect(() => {
    if (isVanilla && (activeTab === 'mods' || activeTab === 'shaders')) {
      setActiveTab('overview')
    } else if (!isDatapackSupported && activeTab === 'datapacks') {
      setActiveTab('overview')
    }
  }, [isVanilla, isDatapackSupported, activeTab])
  const updatesQuery = useContentUpdates(
    id ?? '',
    activeTab === 'mods' ||
      activeTab === 'shaders' ||
      activeTab === 'resourcepacks' ||
      activeTab === 'datapacks',
  )
  const updatesMap = useMemo(() => {
    const map = new Map<string, { latestVersionNumber: string }>()
    for (const u of updatesQuery.data ?? []) {
      map.set(`${u.provider}:${u.projectId}`, u)
    }
    return map
  }, [updatesQuery.data])
  const worldsQuery = useContentWorlds(id ?? '', activeTab === 'worlds')

  // Filter content per kategori
  const mods = useMemo(
    () => (instance?.content ?? []).filter((entry) => entry.type === 'mod'),
    [instance?.content],
  )

  const shaders = useMemo(
    () => (instance?.content ?? []).filter((entry) => entry.type === 'shader'),
    [instance?.content],
  )

  const resourcePacks = useMemo(
    () => (instance?.content ?? []).filter((entry) => entry.type === 'resourcepack'),
    [instance?.content],
  )

  const datapacks = useMemo(
    () => (instance?.content ?? []).filter((entry) => entry.type === 'datapack'),
    [instance?.content],
  )

  const savedWorlds = useMemo(() => worldsQuery.data ?? [], [worldsQuery.data])

  useEffect(() => {
    if (instance) {
      setEditName(instance.name)
      setEditVersion(instance.minecraftVersion)
      setEditLoaderType(instance.loader.type)
      setEditLoaderVersion(instance.loader.version)
    }
  }, [
    instance?.id,
    instance?.name,
    instance?.minecraftVersion,
    instance?.loader.type,
    instance?.loader.version,
  ])

  const releases = useMemo(() => {
    const list = (versionsQuery.data ?? []).filter((v) => v.type === 'release')
    return list.slice().sort((a, b) => compareMinecraftVersions(b.id, a.id))
  }, [versionsQuery.data])

  const versionOptions: GlassSelectOption[] = useMemo(() => {
    return releases.map((v) => ({
      value: v.id,
      label: v.id,
      ...(v.installed ? { badge: t('instance.versionInstalled') } : {}),
    }))
  }, [releases, t])

  const loaderOptions: GlassSelectOption[] = useMemo(
    () => [
      { value: 'vanilla', label: 'Vanilla' },
      { value: 'fabric', label: 'Fabric' },
      { value: 'quilt', label: 'Quilt' },
      { value: 'forge', label: 'Forge' },
      { value: 'neoforge', label: 'NeoForge' },
    ],
    [],
  )

  const resourcePackOptions: GlassSelectOption[] = useMemo(
    () => [
      { value: 'prompt', label: t('instance.serverResourcePackPrompt') },
      { value: 'enabled', label: t('instance.serverResourcePackEnabled') },
      { value: 'disabled', label: t('instance.serverResourcePackDisabled') },
    ],
    [t],
  )

  const servers = useMemo(() => serversQuery.data ?? [], [serversQuery.data])

  const pingSingleServer = useCallback(
    async (ip: string): Promise<void> => {
      if (!id) return
      setServerStatuses((prev) => ({ ...prev, [ip]: 'loading' }))
      try {
        const status = await invoke('servers:ping', { instanceId: id, ip })
        setServerStatuses((prev) => ({ ...prev, [ip]: status }))
      } catch {
        setServerStatuses((prev) => ({
          ...prev,
          [ip]: {
            ip,
            online: false,
            latencyMs: null,
            playersOnline: 0,
            playersMax: 0,
            motd: '',
            versionName: null,
            favicon: null,
            checkedAt: new Date().toISOString(),
          },
        }))
      }
    },
    [id],
  )

  const pingAllServers = useCallback(async (): Promise<void> => {
    if (servers.length === 0) return
    setIsRefreshingServers(true)
    try {
      await Promise.all(servers.map((s) => pingSingleServer(s.ip)))
    } finally {
      setIsRefreshingServers(false)
    }
  }, [servers, pingSingleServer])

  useEffect(() => {
    if (activeTab !== 'servers') {
      autoPingedIpsRef.current.clear()
      return
    }
    for (const srv of servers) {
      if (!autoPingedIpsRef.current.has(srv.ip)) {
        autoPingedIpsRef.current.add(srv.ip)
        void pingSingleServer(srv.ip)
      }
    }
  }, [activeTab, servers, pingSingleServer])

  const handleSaveSettings = async (): Promise<void> => {
    if (!instance) return
    if (!editName.trim()) {
      pushToast(t('createInstance.nameRequired'), 'error')
      return
    }
    if (!editVersion) {
      pushToast(t('createInstance.versionRequired'), 'error')
      return
    }
    try {
      await updateInstance.mutateAsync({
        ...instance,
        name: editName.trim(),
        minecraftVersion: editVersion,
        loader: {
          type: editLoaderType,
          version: editLoaderType === 'vanilla' ? null : editLoaderVersion,
        },
      })
      pushToast(t('instance.saved'), 'info')
    } catch (err) {
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    }
  }

  if (instancesQuery.isLoading) {
    return (
      <div className={styles.page}>
        <p>{t('common.loading')}</p>
      </div>
    )
  }

  if (!instance) {
    return (
      <div className={styles.page}>
        <div className={styles.backRow}>
          <GlassButton
            variant="ghost"
            icon={<ArrowLeft size={14} aria-hidden />}
            onClick={() => navigate('/instances')}
          >
            {t('instance.back')}
          </GlassButton>
        </div>
        <p>{t('common.notFound')}</p>
      </div>
    )
  }

  const handleLaunch = async (serverAddress?: string): Promise<void> => {
    if (!instance.minecraftVersion) {
      pushToast(t('home.play.unavailable'), 'error')
      return
    }
    const tempTaskId = `launch-${Date.now()}`
    setLaunchInstanceId(instance.id)
    setLastLaunchInstanceId(instance.id)
    setLaunchTaskId(tempTaskId)
    try {
      const res = await window.launcher.invoke('launch:start', {
        instanceId: instance.id,
        versionId: instance.minecraftVersion,
        ...(serverAddress ? { serverAddress } : {}),
      })
      if (!res.ok) {
        setLaunchInstanceId(null)
        setLaunchTaskId(null)
        pushToast(res.error.message, 'error')
        return
      }
      setLaunchTaskId(res.data.taskId)
    } catch (err) {
      setLaunchInstanceId(null)
      setLaunchTaskId(null)
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    }
  }

  const handleOpenFolder = (
    subfolder?: 'mods' | 'resourcepacks' | 'saves' | 'shaderpacks',
  ): void => {
    void invoke('instances:openFolder', { id: instance.id, subfolder }).catch((err: unknown) => {
      pushToast(errorMessage(err, t('error.unknown')), 'error')
    })
  }

  const handleDelete = (): void => {
    removeInstance.mutate(instance.id, {
      onSuccess: () => {
        setDeleteOpen(false)
        pushToast(t('instances.deleted'))
        if (deleteGameData) {
          void invoke('downloads:cleanup', undefined)
            .then(({ taskId }) => setGlobalCleanupTaskId(taskId))
            .catch((err: unknown) => {
              pushToast(errorMessage(err, t('error.unknown')), 'error')
            })
        }
        navigate('/instances')
      },
      onError: (err) => {
        pushToast(errorMessage(err, t('error.unknown')), 'error')
      },
    })
  }

  const handleRemoveContentEntry = (
    provider: 'modrinth' | 'curseforge',
    projectId: string,
  ): void => {
    removeContent.mutate(
      { instanceId: instance.id, provider, projectId },
      {
        onSuccess: () => pushToast(t('instance.contentRemoved')),
        onError: (err) =>
          pushToast(errorMessage(err, t('instance.contentRemoveFailed')), 'error'),
      },
    )
  }

  const handleToggleContent = (
    provider: ContentProviderId,
    projectId: string,
    enabled: boolean,
    fileName: string,
  ): void => {
    toggleContent.mutate(
      { instanceId: instance.id, provider, projectId, enabled },
      {
        onSuccess: () => {
          pushToast(
            enabled
              ? t('instance.modEnabledToast', { name: fileName })
              : t('instance.modDisabledToast', { name: fileName }),
          )
        },
        onError: (err) => {
          pushToast(errorMessage(err, t('error.unknown')), 'error')
        },
      },
    )
  }

  // Filter teks untuk pencarian item di dalam tab aktif
  const term = searchFilter.trim().toLowerCase()
  const filteredMods = mods.filter(
    (m) => term === '' || m.fileName.toLowerCase().includes(term),
  )
  const filteredShaders = shaders.filter(
    (s) => term === '' || s.fileName.toLowerCase().includes(term),
  )
  const filteredWorlds = savedWorlds.filter(
    (w) =>
      term === '' ||
      w.name.toLowerCase().includes(term) ||
      w.folder.toLowerCase().includes(term),
  )
  const filteredResourcePacks = resourcePacks.filter(
    (r) => term === '' || r.fileName.toLowerCase().includes(term),
  )
  const filteredDatapacks = datapacks.filter(
    (d) =>
      term === '' ||
      d.fileName.toLowerCase().includes(term) ||
      (d.worldFolder && d.worldFolder.toLowerCase().includes(term)),
  )

  const filteredServers = servers.filter((s) => {
    const matchesSearch =
      term === '' ||
      s.name.toLowerCase().includes(term) ||
      s.ip.toLowerCase().includes(term)
    if (!matchesSearch) return false
    if (serverStatusFilter === 'online') {
      const st = serverStatuses[s.ip]
      return st !== undefined && st !== 'loading' && st.online
    }
    return true
  })

  const openAddServerDialog = () => {
    setServerDialogMode('add')
    setServerNameInput('')
    setServerIpInput('')
    setServerResourcePackInput('prompt')
    setServerHideFromHomeInput(false)
    setEditingOldIp('')
    setServerDialogOpen(true)
  }

  const openEditServerDialog = (server: InstanceServerEntry) => {
    setServerDialogMode('edit')
    setServerNameInput(server.name)
    setServerIpInput(server.ip)
    setServerResourcePackInput(server.resourcePackPolicy ?? 'prompt')
    setServerHideFromHomeInput(Boolean(server.hideFromHome))
    setEditingOldIp(server.ip)
    setServerDialogOpen(true)
  }

  const handleSaveServer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!instance || !serverNameInput.trim() || !serverIpInput.trim()) return

    const trimmedName = serverNameInput.trim()
    const trimmedIp = serverIpInput.trim()

    try {
      if (serverDialogMode === 'add') {
        await addServer.mutateAsync({
          instanceId: instance.id,
          name: trimmedName,
          ip: trimmedIp,
          resourcePackPolicy: serverResourcePackInput,
          hideFromHome: serverHideFromHomeInput,
        })
        pushToast(t('instance.serverAddedToast', { name: trimmedName }), 'info')
        void pingSingleServer(trimmedIp)
      } else {
        await editServer.mutateAsync({
          instanceId: instance.id,
          oldIp: editingOldIp,
          name: trimmedName,
          newIp: trimmedIp,
          resourcePackPolicy: serverResourcePackInput,
          hideFromHome: serverHideFromHomeInput,
        })
        pushToast(t('instance.serverEditedToast', { name: trimmedName }), 'info')
        void pingSingleServer(trimmedIp)
      }
      setServerDialogOpen(false)
    } catch (error) {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    }
  }

  const handleConfirmDeleteServer = async () => {
    if (!instance || !serverDeleteTarget) return
    try {
      await removeServer.mutateAsync({
        instanceId: instance.id,
        ip: serverDeleteTarget.ip,
      })
      pushToast(t('instance.serverDeletedToast', { name: serverDeleteTarget.name }), 'info')
      setServerDeleteTarget(null)
    } catch (error) {
      pushToast(errorMessage(error, t('error.unknown')), 'error')
    }
  }

  const handleCopyServerIp = async (ip: string) => {
    try {
      await navigator.clipboard.writeText(ip)
      setCopiedIp(ip)
      pushToast(t('instance.serverIpCopied'), 'info')
      setTimeout(() => {
        setCopiedIp((prev) => (prev === ip ? null : prev))
      }, 2000)
    } catch {
      // clipboard might fail in test env
    }
  }

  return (
    <div ref={pageRef} className={styles.page} data-testid="instance-detail-screen">
      <div className={styles.backRow}>
        <GlassButton
          variant="ghost"
          icon={<ArrowLeft size={14} aria-hidden />}
          onClick={() => navigate('/instances')}
        >
          {t('instance.back')}
        </GlassButton>
      </div>

      <header className={styles.header}>
        <div className={styles.titleArea}>
          <InstanceIcon instance={instance} size={42} />
          <div>
            <h1 className={styles.name}>{instance.name}</h1>
            <div className={styles.meta}>
              <span>{instance.minecraftVersion}</span>
              <span className={styles.badge}>{instance.loader.type}</span>
              <span>
                {instance.lastPlayedAt
                  ? t('instances.lastPlayed', {
                      date: new Intl.DateTimeFormat(language, { dateStyle: 'medium' }).format(
                        new Date(instance.lastPlayedAt),
                      ),
                    })
                  : t('instances.neverPlayed')}
              </span>
            </div>
          </div>
        </div>

        <div className={styles.headerActions}>
          <GlassButton
            variant="outline"
            icon={<FolderOpen size={14} aria-hidden />}
            onClick={() => {
              void invoke('instances:openFolder', { id: instance.id })
            }}
            title={t('instances.openFolder')}
          >
            {t('instances.openFolder')}
          </GlassButton>
          <GlassButton
            variant="primary"
            icon={<Play size={14} aria-hidden />}
            disabled={
              launchTaskId !== null ||
              !instance.minecraftVersion ||
              isGameRunning
            }
            onClick={() => void handleLaunch()}
          >
            {launchTaskId && launchInstanceId === instance.id
              ? t('home.launching')
              : isGameRunning && launchInstanceId === instance.id
                ? t('home.running')
                : t('instances.play')}
          </GlassButton>
        </div>
      </header>

      {/* Navigasi Tab Manajemen Khusus Per Instance */}
      <div className={styles.tabs} role="tablist">
        {/* Tab 1: Ringkasan */}
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'overview'}
          className={styles.tab}
          onClick={() => {
            setActiveTab('overview')
            setSearchFilter('')
          }}
          data-testid="instance-tab-overview"
        >
          <span className={styles.tabLabel}>{t('instance.tabs.overview')}</span>
        </button>

        {/* Tab 2: Mods (Hanya untuk loader modded seperti Fabric, Forge, Quilt, NeoForge) */}
        {!isVanilla ? (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'mods'}
            className={styles.tab}
            onClick={() => {
              setActiveTab('mods')
              setSearchFilter('')
            }}
            data-testid="instance-tab-mods"
          >
            <span className={styles.tabLabel}>
              {t('instance.tabs.mods')}
              {mods.length > 0 ? <span className={styles.tabBadge}>{mods.length}</span> : null}
            </span>
          </button>
        ) : null}

        {/* Tab 3: Resource Packs */}
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'resourcepacks'}
          className={styles.tab}
          onClick={() => {
            setActiveTab('resourcepacks')
            setSearchFilter('')
          }}
          data-testid="instance-tab-resourcepacks"
        >
          <span className={styles.tabLabel}>
            {t('instance.tabs.resourcepacks')}
            {resourcePacks.length > 0 ? (
              <span className={styles.tabBadge}>{resourcePacks.length}</span>
            ) : null}
          </span>
        </button>

        {/* Tab 4: Shaders (Hanya untuk loader modded) */}
        {!isVanilla ? (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'shaders'}
            className={styles.tab}
            onClick={() => {
              setActiveTab('shaders')
              setSearchFilter('')
            }}
            data-testid="instance-tab-shaders"
          >
            <span className={styles.tabLabel}>
              {t('instance.tabs.shaders')}
              {shaders.length > 0 ? (
                <span className={styles.tabBadge}>{shaders.length}</span>
              ) : null}
            </span>
          </button>
        ) : null}

        {/* Tab 5: Datapacks (Hanya untuk Minecraft 1.13 ke atas) */}
        {isDatapackSupported ? (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'datapacks'}
            className={styles.tab}
            onClick={() => {
              setActiveTab('datapacks')
              setSearchFilter('')
            }}
            data-testid="instance-tab-datapacks"
          >
            <span className={styles.tabLabel}>
              {t('instance.tabs.datapacks')}
              {datapacks.length > 0 ? (
                <span className={styles.tabBadge}>{datapacks.length}</span>
              ) : null}
            </span>
          </button>
        ) : null}

        {/* Tab 6: Worlds */}
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'worlds'}
          className={styles.tab}
          onClick={() => {
            setActiveTab('worlds')
            setSearchFilter('')
          }}
          data-testid="instance-tab-worlds"
        >
          <span className={styles.tabLabel}>
            {t('instance.tabs.worlds')}
            {savedWorlds.length > 0 ? (
              <span className={styles.tabBadge}>{savedWorlds.length}</span>
            ) : null}
          </span>
        </button>

        {/* Tab 7: Servers */}
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'servers'}
          className={styles.tab}
          onClick={() => {
            setActiveTab('servers')
            setSearchFilter('')
          }}
          data-testid="instance-tab-servers"
        >
          <span className={styles.tabLabel}>
            <Server size={14} aria-hidden />
            {t('instance.tabs.servers')}
            {servers.length > 0 ? (
              <span className={styles.tabBadge}>{servers.length}</span>
            ) : null}
          </span>
        </button>

        {/* Tab 8: Settings */}
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'settings'}
          className={styles.tab}
          onClick={() => {
            setActiveTab('settings')
            setSearchFilter('')
          }}
          data-testid="instance-tab-settings"
        >
          <span className={styles.tabLabel}>
            <SettingsIcon size={14} aria-hidden />
            {t('instance.settings')}
          </span>
        </button>
      </div>

      <div className={styles.content}>
        {/* ===================== TAB 1: OVERVIEW ===================== */}
        {activeTab === 'overview' ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.field}>
              <span className={styles.fieldLabel}>{t('instance.name')}</span>
              <span>{instance.name}</span>
            </div>
            <div className={styles.field}>
              <span className={styles.fieldLabel}>{t('instance.version')}</span>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <span data-testid="instance-detail-version">
                  {instance.minecraftVersion || t('instance.versionUnavailable')}
                </span>
                <GlassButton
                  size="sm"
                  variant="ghost"
                  onClick={() => setActiveTab('settings')}
                  data-testid="instance-edit-version-btn"
                >
                  {t('instance.editVersion')}
                </GlassButton>
              </div>
            </div>
            <div className={styles.field}>
              <span className={styles.fieldLabel}>{t('instance.loader')}</span>
              <span>
                {instance.loader.type}
                {instance.loader.version ? ` ${instance.loader.version}` : ''}
              </span>
            </div>

            <div className={styles.deleteArea}>
              <GlassButton
                variant="ghost"
                icon={<FolderOpen size={14} aria-hidden />}
                onClick={() => handleOpenFolder()}
                data-testid="instance-open-folder"
              >
                {t('instances.openFolder')}
              </GlassButton>
              <GlassButton
                variant="ghost"
                icon={<Trash2 size={14} aria-hidden />}
                onClick={() => setDeleteOpen(true)}
                data-testid="instance-delete-button"
              >
                {t('instances.delete')}
              </GlassButton>
            </div>
          </GlassCard>
        ) : null}

        {/* ===================== TAB 2: MODS ===================== */}
        {activeTab === 'mods' && !isVanilla ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.subHeader}>
              <div className={styles.subHeaderLeft}>
                <div className={styles.modeToggleGroup}>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'manager' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('manager')}
                  >
                    <SlidersHorizontal size={13} aria-hidden />
                    <span>
                      {t('instance.viewManager')} ({mods.length})
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'downloader' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('downloader')}
                  >
                    <Download size={13} aria-hidden />
                    <span>{t('instance.viewDownloader')}</span>
                  </button>
                </div>

                {viewMode === 'manager' && mods.length > 3 ? (
                  <div className={styles.filterBox}>
                    <Search size={13} className={styles.filterIcon} aria-hidden />
                    <input
                      type="text"
                      className={styles.filterInput}
                      placeholder={t('instance.searchPlaceholder')}
                      value={searchFilter}
                      onChange={(e) => setSearchFilter(e.target.value)}
                    />
                  </div>
                ) : null}
              </div>

              {viewMode === 'manager' ? (
                <div className={styles.subHeaderActions}>
                  {mods.length > 0 ? (
                    <GlassButton
                      variant="ghost"
                      size="sm"
                      icon={
                        updatesQuery.isFetching ? (
                          <Spinner size={13} />
                        ) : (
                          <RefreshCw size={13} aria-hidden />
                        )
                      }
                      disabled={updatesQuery.isFetching}
                      onClick={() => void updatesQuery.refetch()}
                      title={t('instance.checkUpdates')}
                    >
                      {updatesQuery.isFetching
                        ? t('instance.checkingUpdates')
                        : t('instance.checkUpdates')}
                    </GlassButton>
                  ) : null}
                  <GlassButton
                    variant="outline"
                    size="sm"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('mods')}
                    title={t('instance.openModsFolder')}
                  >
                    {t('instance.openModsFolder')}
                  </GlassButton>
                  <GlassButton
                    variant="primary"
                    size="sm"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              ) : null}
            </div>

            {viewMode === 'downloader' ? (
              <InstanceDownloader instance={instance} type="mod" />
            ) : mods.length === 0 ? (
              <div className={styles.emptyState}>
                <div className={styles.emptyIcon}>
                  <Puzzle size={22} aria-hidden />
                </div>
                <h3 className={styles.emptyTitle}>{t('instance.modsEmpty')}</h3>
                <p className={styles.emptyHint}>{t('instance.modsEmptyHint')}</p>
                <div className={styles.emptyActions}>
                  <GlassButton
                    variant="primary"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                  <GlassButton
                    variant="ghost"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('mods')}
                  >
                    {t('instance.openModsFolder')}
                  </GlassButton>
                </div>
              </div>
            ) : filteredMods.length > 25 ? (
              <VirtualizedContentList
                items={filteredMods}
                updatesMap={updatesMap}
                icon={<Puzzle size={16} aria-hidden />}
                scrollContainerRef={pageRef}
                isGameRunning={isGameRunning}
                togglePending={toggleContent.isPending}
                removePending={removeContent.isPending}
                onToggle={(provider, projectId, checked, fileName) =>
                  handleToggleContent(provider, projectId, checked, fileName)
                }
                onRemove={(provider, projectId) =>
                  handleRemoveContentEntry(provider, projectId)
                }
                t={t}
              />
            ) : (
              <ul className={styles.contentList}>
                {filteredMods.map((entry) => {
                  const update = updatesMap.get(`${entry.provider}:${entry.projectId}`)
                  return (
                    <li
                      key={`${entry.provider}:${entry.projectId}`}
                      className={`${styles.contentRow} ${entry.disabled ? styles.contentRowDisabled : ''}`}
                    >
                      <div className={styles.itemDetails}>
                        <div className={`${styles.itemIcon} ${entry.disabled ? styles.itemIconDisabled : ''}`}>
                          <Puzzle size={16} aria-hidden />
                        </div>
                        <div className={styles.itemText}>
                          <div className={styles.contentTitleRow}>
                            <span
                              className={`${styles.contentName} ${entry.disabled ? styles.contentNameDisabled : ''}`}
                              title={entry.fileName}
                            >
                              {entry.fileName}
                            </span>
                            <span className={entry.disabled ? styles.statusBadgeDisabled : styles.statusBadgeActive}>
                              {entry.disabled ? t('instance.modDisabled') : t('instance.modEnabled')}
                            </span>
                            {update ? (
                              <span
                                className={styles.updateBadge}
                                title={t('instance.updateAvailable', {
                                  version: update.latestVersionNumber,
                                })}
                              >
                                {t('instance.updateBadge', {
                                  version: update.latestVersionNumber,
                                })}
                              </span>
                            ) : null}
                          </div>
                          <span className={styles.contentMeta}>
                            {t(CONTENT_TYPE_KEYS[entry.type])} · {entry.provider}
                          </span>
                        </div>
                      </div>
                      <div className={styles.itemActions}>
                        <div className={styles.toggleWrap}>
                          <GlassSwitch
                            checked={!entry.disabled}
                            onCheckedChange={(checked) =>
                              handleToggleContent(entry.provider, entry.projectId, checked, entry.fileName)
                            }
                            label={entry.disabled ? t('instance.enableMod') : t('instance.disableMod')}
                            disabled={toggleContent.isPending || isGameRunning}
                          />
                          <span
                            className={`${styles.toggleLabel} ${entry.disabled ? styles.toggleLabelDisabled : styles.toggleLabelActive}`}
                          >
                            {entry.disabled ? t('instance.modDisabled') : t('instance.modEnabled')}
                          </span>
                        </div>
                        <GlassButton
                          variant="ghost"
                          size="sm"
                          icon={<Trash2 size={13} aria-hidden />}
                          disabled={removeContent.isPending || isGameRunning}
                          onClick={() => handleRemoveContentEntry(entry.provider, entry.projectId)}
                          aria-label={t('instance.contentRemove')}
                          title={t('instance.contentRemove')}
                        >
                          {t('instance.contentRemove')}
                        </GlassButton>
                      </div>
                    </li>
                  )
                })}
              </ul>
            )}
          </GlassCard>
        ) : null}

        {/* ===================== TAB 3: WORLDS ===================== */}
        {activeTab === 'worlds' ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.subHeader}>
              <div className={styles.subHeaderLeft}>
                <div className={styles.modeToggleGroup}>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'manager' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('manager')}
                  >
                    <SlidersHorizontal size={13} aria-hidden />
                    <span>
                      {t('instance.viewManager')} ({savedWorlds.length})
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'downloader' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('downloader')}
                  >
                    <Download size={13} aria-hidden />
                    <span>{t('instance.viewDownloader')}</span>
                  </button>
                </div>

                {viewMode === 'manager' && savedWorlds.length > 3 ? (
                  <div className={styles.filterBox}>
                    <Search size={13} className={styles.filterIcon} aria-hidden />
                    <input
                      type="text"
                      className={styles.filterInput}
                      placeholder={t('instance.searchPlaceholder')}
                      value={searchFilter}
                      onChange={(e) => setSearchFilter(e.target.value)}
                    />
                  </div>
                ) : null}
              </div>

              {viewMode === 'manager' ? (
                <div className={styles.subHeaderActions}>
                  <GlassButton
                    variant="outline"
                    size="sm"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('saves')}
                    title={t('instance.openWorldsFolder')}
                  >
                    {t('instance.openWorldsFolder')}
                  </GlassButton>
                  <GlassButton
                    variant="primary"
                    size="sm"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              ) : null}
            </div>

            {viewMode === 'downloader' ? (
              <InstanceDownloader instance={instance} type="world" />
            ) : savedWorlds.length === 0 ? (
              <div className={styles.emptyState}>
                <div className={styles.emptyIcon}>
                  <Globe size={22} aria-hidden />
                </div>
                <h3 className={styles.emptyTitle}>{t('instance.worldsEmpty')}</h3>
                <p className={styles.emptyHint}>{t('instance.worldsEmptyHint')}</p>
                <div className={styles.emptyActions}>
                  <GlassButton
                    variant="primary"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                  <GlassButton
                    variant="ghost"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('saves')}
                  >
                    {t('instance.openWorldsFolder')}
                  </GlassButton>
                </div>
              </div>
            ) : (
              <ul className={styles.contentList}>
                {filteredWorlds.map((world) => {
                  const contentEntry = instance.content.find(
                    (c) => c.type === 'world' && c.worldFolder === world.folder,
                  )
                  return (
                    <li key={world.folder} className={styles.contentRow}>
                      <div className={styles.itemDetails}>
                        <div className={styles.itemIcon}>
                          <Globe size={16} aria-hidden />
                        </div>
                        <div className={styles.itemText}>
                          <div className={styles.contentTitleRow}>
                            <span className={styles.contentName}>{world.name}</span>
                            <span className={styles.worldBadge}>saves/{world.folder}</span>
                          </div>
                          <span className={styles.contentMeta}>
                            {contentEntry
                              ? `${t('instance.contentType.world')} · ${contentEntry.provider}`
                              : t('instance.contentType.world')}
                          </span>
                        </div>
                      </div>

                      {contentEntry ? (
                        <GlassButton
                          variant="ghost"
                          size="sm"
                          icon={<Trash2 size={13} aria-hidden />}
                          disabled={removeContent.isPending || isGameRunning}
                          onClick={() =>
                            handleRemoveContentEntry(
                              contentEntry.provider,
                              contentEntry.projectId,
                            )
                          }
                          aria-label={t('instance.contentRemove')}
                          title={t('instance.contentRemove')}
                        >
                          {t('instance.contentRemove')}
                        </GlassButton>
                      ) : null}
                    </li>
                  )
                })}
              </ul>
            )}
          </GlassCard>
        ) : null}

        {/* ===================== TAB: SERVERS ===================== */}
        {activeTab === 'servers' ? (
          <div className={styles.serverSection}>
            <div className={styles.serverToolbar}>
              <div className={styles.serverSearchBox}>
                <Search size={15} className={styles.serverSearchIcon} aria-hidden />
                <input
                  type="text"
                  className={styles.serverSearchInput}
                  placeholder={t('instance.serverSearchPlaceholder', { count: servers.length })}
                  value={searchFilter}
                  onChange={(e) => setSearchFilter(e.target.value)}
                />
              </div>

              <button
                type="button"
                className={styles.serverAddBtn}
                onClick={openAddServerDialog}
                disabled={isGameRunning}
                data-testid="instance-add-server-btn"
              >
                <Plus size={15} aria-hidden />
                <span>{t('instance.addServer')}</span>
              </button>
            </div>

            <div className={styles.serverFilterBar}>
              <div className={styles.serverFilterLeft}>
                <Filter size={15} className={styles.serverFilterIcon} aria-hidden />
                <button
                  type="button"
                  className={`${styles.serverFilterChip} ${serverStatusFilter === 'all' ? styles.serverFilterChipActive : ''}`}
                  onClick={() => setServerStatusFilter('all')}
                >
                  {t('instance.serverFilterAll')}
                </button>
                <button
                  type="button"
                  className={`${styles.serverFilterChip} ${serverStatusFilter === 'online' ? styles.serverFilterChipActive : ''}`}
                  onClick={() => setServerStatusFilter('online')}
                >
                  {t('instance.serverFilterOnline')}
                </button>
              </div>

              <button
                type="button"
                className={styles.serverRefreshBtn}
                onClick={() => void pingAllServers()}
                disabled={isRefreshingServers || servers.length === 0}
              >
                <RefreshCw
                  size={14}
                  className={isRefreshingServers ? styles.spinning : ''}
                  aria-hidden
                />
                <span>{t('instance.serverRefresh')}</span>
              </button>
            </div>

            {servers.length === 0 ? (
              <GlassCard tone="strong" className={styles.card}>
                <div className={styles.emptyState}>
                  <div className={styles.emptyIcon}>
                    <Server size={22} aria-hidden />
                  </div>
                  <h3 className={styles.emptyTitle}>{t('instance.serversEmpty')}</h3>
                  <p className={styles.emptyHint}>{t('instance.serversEmptyHint')}</p>
                  <div className={styles.emptyActions}>
                    <GlassButton
                      variant="primary"
                      icon={<Plus size={13} aria-hidden />}
                      onClick={openAddServerDialog}
                      disabled={isGameRunning}
                    >
                      {t('instance.addServer')}
                    </GlassButton>
                  </div>
                </div>
              </GlassCard>
            ) : filteredServers.length === 0 ? (
              <GlassCard tone="strong" className={styles.card}>
                <div className={styles.emptyState}>
                  <p className={styles.emptyHint}>{t('instance.serversEmpty')}</p>
                </div>
              </GlassCard>
            ) : (
              <ul className={styles.serverCardList}>
                {filteredServers.map((server) => {
                  const statusEntry = serverStatuses[server.ip]
                  const isPinging = statusEntry === 'loading' || statusEntry === undefined
                  const pingData =
                    statusEntry && statusEntry !== 'loading' ? statusEntry : null
                  const isOnline = Boolean(pingData?.online)
                  const activeSignalBars = !isOnline
                    ? 0
                    : pingData?.latencyMs === null || pingData?.latencyMs === undefined
                      ? 4
                      : pingData.latencyMs < 75
                        ? 4
                        : pingData.latencyMs < 150
                          ? 3
                          : pingData.latencyMs < 300
                            ? 2
                            : 1
                  const resolvedIcon = pingData?.favicon ?? server.icon ?? null
                  const previewMotd =
                    pingData?.motd && pingData.motd.trim().length > 0
                      ? pingData.motd.trim()
                      : server.name

                  return (
                    <li key={server.ip} className={styles.serverCardRow}>
                      <div className={styles.serverLeftSection}>
                        <div className={styles.serverIconBox}>
                          {resolvedIcon ? (
                            <img
                              src={resolvedIcon}
                              alt=""
                              className={styles.serverFaviconLarge}
                              width={48}
                              height={48}
                            />
                          ) : (
                            <Server size={22} aria-hidden />
                          )}
                        </div>

                        <div className={styles.serverInfoCol}>
                          <div className={styles.serverTitleLine}>
                            <span className={styles.serverTitleText} title={server.name}>
                              {server.name}
                            </span>

                            <button
                              type="button"
                              className={styles.serverLinkPill}
                              onClick={() => void handleCopyServerIp(server.ip)}
                              title={`${t('instance.serverCopyIp')}: ${server.ip}`}
                              aria-label={`${t('instance.serverCopyIp')}: ${server.ip}`}
                            >
                              {copiedIp === server.ip ? (
                                <Check size={13} aria-hidden />
                              ) : (
                                <Link2 size={13} aria-hidden />
                              )}
                            </button>

                            <div
                              className={styles.serverStatusBadge}
                              title={
                                pingData?.latencyMs !== null && pingData?.latencyMs !== undefined
                                  ? `${pingData.latencyMs} ms`
                                  : undefined
                              }
                            >
                              {isPinging ? (
                                <>
                                  <Spinner size={13} />
                                  <span className={styles.serverStatusTextMuted}>
                                    {t('instance.serverPinging')}
                                  </span>
                                </>
                              ) : (
                                <>
                                  <span
                                    className={`${styles.signalBars} ${!isOnline ? styles.signalBarsOffline : ''}`}
                                    aria-hidden
                                  >
                                    {[1, 2, 3, 4].map((barIdx) => (
                                      <span
                                        key={barIdx}
                                        className={`${styles.signalBar} ${styles[`signalBar${barIdx}`]} ${barIdx <= activeSignalBars ? styles.signalBarActive : ''}`}
                                      />
                                    ))}
                                  </span>
                                  <span
                                    className={
                                      isOnline
                                        ? styles.serverStatusTextOnline
                                        : styles.serverStatusTextOffline
                                    }
                                  >
                                    {isOnline
                                      ? t('instance.serverOnlineCount', {
                                          online: pingData?.playersOnline ?? 0,
                                        })
                                      : t('instance.serverOffline')}
                                  </span>
                                </>
                              )}
                            </div>
                          </div>

                          <span className={styles.serverSubLine}>
                            {t('instance.serverNotPlayedYet')}
                          </span>
                        </div>
                      </div>

                      <div
                        className={styles.serverPreviewCenter}
                        title={
                          pingData?.versionName
                            ? `${previewMotd} (${pingData.versionName})`
                            : previewMotd
                        }
                      >
                        <span className={styles.serverPreviewMotd}>{previewMotd}</span>
                      </div>

                      <div className={styles.serverRightActions}>
                        <button
                          type="button"
                          className={styles.serverPlayBtn}
                          disabled={
                            launchTaskId !== null ||
                            !instance.minecraftVersion ||
                            isGameRunning
                          }
                          onClick={() => void handleLaunch(server.ip)}
                        >
                          <Play size={15} aria-hidden />
                          <span>{t('instance.serverPlay')}</span>
                        </button>

                        <DropdownMenu.Root>
                          <DropdownMenu.Trigger asChild>
                            <button
                              type="button"
                              className={styles.serverMenuBtn}
                              aria-label={t('instance.serverMoreActions')}
                              title={t('instance.serverMoreActions')}
                            >
                              <MoreVertical size={16} aria-hidden />
                            </button>
                          </DropdownMenu.Trigger>

                          <DropdownMenu.Portal>
                            <DropdownMenu.Content
                              className={styles.serverDropdownContent}
                              align="end"
                              sideOffset={6}
                            >
                              <DropdownMenu.Item
                                className={styles.serverDropdownItem}
                                onSelect={() => void pingSingleServer(server.ip)}
                              >
                                <RefreshCw size={15} aria-hidden />
                                <span>{t('instance.serverRefresh')}</span>
                              </DropdownMenu.Item>

                              <DropdownMenu.Item
                                className={styles.serverDropdownItem}
                                onSelect={() => void handleCopyServerIp(server.ip)}
                              >
                                <Copy size={15} aria-hidden />
                                <span>{t('instance.serverCopyIp')}</span>
                              </DropdownMenu.Item>

                              <DropdownMenu.Item
                                className={styles.serverDropdownItem}
                                disabled={isGameRunning}
                                onSelect={() => openEditServerDialog(server)}
                              >
                                <Pencil size={15} aria-hidden />
                                <span>{t('instance.editServer')}</span>
                              </DropdownMenu.Item>

                              <DropdownMenu.Separator className={styles.serverDropdownSeparator} />

                              <DropdownMenu.Item
                                className={`${styles.serverDropdownItem} ${styles.serverDropdownDanger}`}
                                disabled={removeServer.isPending || isGameRunning}
                                onSelect={() => setServerDeleteTarget(server)}
                              >
                                <Trash2 size={15} aria-hidden />
                                <span>{t('instance.contentRemove')}</span>
                              </DropdownMenu.Item>
                            </DropdownMenu.Content>
                          </DropdownMenu.Portal>
                        </DropdownMenu.Root>
                      </div>
                    </li>
                  )
                })}
              </ul>
            )}
          </div>
        ) : null}

        {/* ===================== TAB 4: RESOURCE PACKS ===================== */}
        {activeTab === 'resourcepacks' ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.subHeader}>
              <div className={styles.subHeaderLeft}>
                <div className={styles.modeToggleGroup}>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'manager' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('manager')}
                  >
                    <SlidersHorizontal size={13} aria-hidden />
                    <span>
                      {t('instance.viewManager')} ({resourcePacks.length})
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'downloader' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('downloader')}
                  >
                    <Download size={13} aria-hidden />
                    <span>{t('instance.viewDownloader')}</span>
                  </button>
                </div>

                {viewMode === 'manager' && resourcePacks.length > 3 ? (
                  <div className={styles.filterBox}>
                    <Search size={13} className={styles.filterIcon} aria-hidden />
                    <input
                      type="text"
                      className={styles.filterInput}
                      placeholder={t('instance.searchPlaceholder')}
                      value={searchFilter}
                      onChange={(e) => setSearchFilter(e.target.value)}
                    />
                  </div>
                ) : null}
              </div>

              {viewMode === 'manager' ? (
                <div className={styles.subHeaderActions}>
                  {resourcePacks.length > 0 ? (
                    <GlassButton
                      variant="ghost"
                      size="sm"
                      icon={
                        updatesQuery.isFetching ? (
                          <Spinner size={13} />
                        ) : (
                          <RefreshCw size={13} aria-hidden />
                        )
                      }
                      disabled={updatesQuery.isFetching}
                      onClick={() => void updatesQuery.refetch()}
                      title={t('instance.checkUpdates')}
                    >
                      {updatesQuery.isFetching
                        ? t('instance.checkingUpdates')
                        : t('instance.checkUpdates')}
                    </GlassButton>
                  ) : null}
                  <GlassButton
                    variant="outline"
                    size="sm"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('resourcepacks')}
                    title={t('instance.openResourcePacksFolder')}
                  >
                    {t('instance.openResourcePacksFolder')}
                  </GlassButton>
                  <GlassButton
                    variant="primary"
                    size="sm"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              ) : null}
            </div>

            {viewMode === 'downloader' ? (
              <InstanceDownloader instance={instance} type="resourcepack" />
            ) : resourcePacks.length === 0 ? (
              <div className={styles.emptyState}>
                <div className={styles.emptyIcon}>
                  <Palette size={22} aria-hidden />
                </div>
                <h3 className={styles.emptyTitle}>{t('instance.resourcepacksEmpty')}</h3>
                <p className={styles.emptyHint}>{t('instance.resourcepacksEmptyHint')}</p>
                <div className={styles.emptyActions}>
                  <GlassButton
                    variant="primary"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                  <GlassButton
                    variant="ghost"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('resourcepacks')}
                  >
                    {t('instance.openResourcePacksFolder')}
                  </GlassButton>
                </div>
              </div>
            ) : (
              <ul className={styles.contentList}>
                {filteredResourcePacks.map((entry) => {
                  const update = updatesQuery.data?.find(
                    (u) => u.projectId === entry.projectId && u.provider === entry.provider,
                  )
                  return (
                    <li key={`${entry.provider}:${entry.projectId}`} className={styles.contentRow}>
                      <div className={styles.itemDetails}>
                        <div className={styles.itemIcon}>
                          <Palette size={16} aria-hidden />
                        </div>
                        <div className={styles.itemText}>
                          <div className={styles.contentTitleRow}>
                            <span className={styles.contentName} title={entry.fileName}>
                              {entry.fileName}
                            </span>
                            {update ? (
                              <span
                                className={styles.updateBadge}
                                title={t('instance.updateAvailable', {
                                  version: update.latestVersionNumber,
                                })}
                              >
                                {t('instance.updateBadge', {
                                  version: update.latestVersionNumber,
                                })}
                              </span>
                            ) : null}
                          </div>
                          <span className={styles.contentMeta}>
                            {t(CONTENT_TYPE_KEYS[entry.type])} · {entry.provider}
                          </span>
                        </div>
                      </div>
                      <GlassButton
                        variant="ghost"
                        size="sm"
                        icon={<Trash2 size={13} aria-hidden />}
                        disabled={removeContent.isPending || isGameRunning}
                        onClick={() => handleRemoveContentEntry(entry.provider, entry.projectId)}
                        aria-label={t('instance.contentRemove')}
                        title={t('instance.contentRemove')}
                      >
                        {t('instance.contentRemove')}
                      </GlassButton>
                    </li>
                  )
                })}
              </ul>
            )}
          </GlassCard>
        ) : null}

        {/* ===================== TAB: SHADERS ===================== */}
        {activeTab === 'shaders' && !isVanilla ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.subHeader}>
              <div className={styles.subHeaderLeft}>
                <div className={styles.modeToggleGroup}>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'manager' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('manager')}
                  >
                    <SlidersHorizontal size={13} aria-hidden />
                    <span>
                      {t('instance.viewManager')} ({shaders.length})
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'downloader' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('downloader')}
                  >
                    <Download size={13} aria-hidden />
                    <span>{t('instance.viewDownloader')}</span>
                  </button>
                </div>

                {viewMode === 'manager' && shaders.length > 3 ? (
                  <div className={styles.filterBox}>
                    <Search size={13} className={styles.filterIcon} aria-hidden />
                    <input
                      type="text"
                      className={styles.filterInput}
                      placeholder={t('instance.searchPlaceholder')}
                      value={searchFilter}
                      onChange={(e) => setSearchFilter(e.target.value)}
                    />
                  </div>
                ) : null}
              </div>

              {viewMode === 'manager' ? (
                <div className={styles.subHeaderActions}>
                  {shaders.length > 0 ? (
                    <GlassButton
                      variant="ghost"
                      size="sm"
                      icon={
                        updatesQuery.isFetching ? (
                          <Spinner size={13} />
                        ) : (
                          <RefreshCw size={13} aria-hidden />
                        )
                      }
                      disabled={updatesQuery.isFetching}
                      onClick={() => void updatesQuery.refetch()}
                      title={t('instance.checkUpdates')}
                    >
                      {updatesQuery.isFetching
                        ? t('instance.checkingUpdates')
                        : t('instance.checkUpdates')}
                    </GlassButton>
                  ) : null}
                  <GlassButton
                    variant="outline"
                    size="sm"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('shaderpacks')}
                    title={t('instance.openShadersFolder')}
                  >
                    {t('instance.openShadersFolder')}
                  </GlassButton>
                  <GlassButton
                    variant="primary"
                    size="sm"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              ) : null}
            </div>

            {viewMode === 'downloader' ? (
              <InstanceDownloader instance={instance} type="shader" />
            ) : shaders.length === 0 ? (
              <div className={styles.emptyState}>
                <div className={styles.emptyIcon}>
                  <Sun size={22} aria-hidden />
                </div>
                <h3 className={styles.emptyTitle}>{t('instance.shadersEmpty')}</h3>
                <p className={styles.emptyHint}>{t('instance.shadersEmptyHint')}</p>
                <div className={styles.emptyActions}>
                  <GlassButton
                    variant="primary"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                  <GlassButton
                    variant="ghost"
                    icon={<FolderOpen size={13} aria-hidden />}
                    onClick={() => handleOpenFolder('shaderpacks')}
                  >
                    {t('instance.openShadersFolder')}
                  </GlassButton>
                </div>
              </div>
            ) : (
              <ul className={styles.contentList}>
                {filteredShaders.map((entry) => {
                  const update = updatesQuery.data?.find(
                    (u) => u.projectId === entry.projectId && u.provider === entry.provider,
                  )
                  return (
                    <li key={`${entry.provider}:${entry.projectId}`} className={styles.contentRow}>
                      <div className={styles.itemDetails}>
                        <div className={styles.itemIcon}>
                          <Sun size={16} aria-hidden />
                        </div>
                        <div className={styles.itemText}>
                          <div className={styles.contentTitleRow}>
                            <span className={styles.contentName} title={entry.fileName}>
                              {entry.fileName}
                            </span>
                            {update ? (
                              <span
                                className={styles.updateBadge}
                                title={t('instance.updateAvailable', {
                                  version: update.latestVersionNumber,
                                })}
                              >
                                {t('instance.updateBadge', {
                                  version: update.latestVersionNumber,
                                })}
                              </span>
                            ) : null}
                          </div>
                          <span className={styles.contentMeta}>
                            {t(CONTENT_TYPE_KEYS[entry.type])} · {entry.provider}
                          </span>
                        </div>
                      </div>
                      <GlassButton
                        variant="ghost"
                        size="sm"
                        icon={<Trash2 size={13} aria-hidden />}
                        disabled={removeContent.isPending || isGameRunning}
                        onClick={() => handleRemoveContentEntry(entry.provider, entry.projectId)}
                        aria-label={t('instance.contentRemove')}
                        title={t('instance.contentRemove')}
                      >
                        {t('instance.contentRemove')}
                      </GlassButton>
                    </li>
                  )
                })}
              </ul>
            )}
          </GlassCard>
        ) : null}

        {/* ===================== TAB 5: DATAPACKS ===================== */}
        {isDatapackSupported && activeTab === 'datapacks' ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.subHeader}>
              <div className={styles.subHeaderLeft}>
                <div className={styles.modeToggleGroup}>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'manager' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('manager')}
                  >
                    <SlidersHorizontal size={13} aria-hidden />
                    <span>
                      {t('instance.viewManager')} ({datapacks.length})
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`${styles.modeBtn} ${viewMode === 'downloader' ? styles.modeBtnActive : ''}`}
                    onClick={() => setViewMode('downloader')}
                  >
                    <Download size={13} aria-hidden />
                    <span>{t('instance.viewDownloader')}</span>
                  </button>
                </div>

                {viewMode === 'manager' && datapacks.length > 3 ? (
                  <div className={styles.filterBox}>
                    <Search size={13} className={styles.filterIcon} aria-hidden />
                    <input
                      type="text"
                      className={styles.filterInput}
                      placeholder={t('instance.searchPlaceholder')}
                      value={searchFilter}
                      onChange={(e) => setSearchFilter(e.target.value)}
                    />
                  </div>
                ) : null}
              </div>

              {viewMode === 'manager' ? (
                <div className={styles.subHeaderActions}>
                  {datapacks.length > 0 ? (
                    <GlassButton
                      variant="ghost"
                      size="sm"
                      icon={
                        updatesQuery.isFetching ? (
                          <Spinner size={13} />
                        ) : (
                          <RefreshCw size={13} aria-hidden />
                        )
                      }
                      disabled={updatesQuery.isFetching}
                      onClick={() => void updatesQuery.refetch()}
                      title={t('instance.checkUpdates')}
                    >
                      {updatesQuery.isFetching
                        ? t('instance.checkingUpdates')
                        : t('instance.checkUpdates')}
                    </GlassButton>
                  ) : null}
                  <GlassButton
                    variant="primary"
                    size="sm"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              ) : null}
            </div>

            {viewMode === 'downloader' ? (
              <InstanceDownloader instance={instance} type="datapack" />
            ) : datapacks.length === 0 ? (
              <div className={styles.emptyState}>
                <div className={styles.emptyIcon}>
                  <FileCode size={22} aria-hidden />
                </div>
                <h3 className={styles.emptyTitle}>{t('instance.datapacksEmpty')}</h3>
                <p className={styles.emptyHint}>{t('instance.datapacksEmptyHint')}</p>
                <div className={styles.emptyActions}>
                  <GlassButton
                    variant="primary"
                    icon={<Download size={13} aria-hidden />}
                    onClick={() => setViewMode('downloader')}
                  >
                    {t('instance.openDownloader')}
                  </GlassButton>
                </div>
              </div>
            ) : (
              <ul className={styles.contentList}>
                {filteredDatapacks.map((entry) => {
                  const update = updatesQuery.data?.find(
                    (u) => u.projectId === entry.projectId && u.provider === entry.provider,
                  )
                  return (
                    <li key={`${entry.provider}:${entry.projectId}`} className={styles.contentRow}>
                      <div className={styles.itemDetails}>
                        <div className={styles.itemIcon}>
                          <FileCode size={16} aria-hidden />
                        </div>
                        <div className={styles.itemText}>
                          <div className={styles.contentTitleRow}>
                            <span className={styles.contentName} title={entry.fileName}>
                              {entry.fileName}
                            </span>
                            {entry.worldFolder ? (
                              <span className={styles.worldBadge}>
                                saves/{entry.worldFolder}/datapacks
                              </span>
                            ) : null}
                            {update ? (
                              <span
                                className={styles.updateBadge}
                                title={t('instance.updateAvailable', {
                                  version: update.latestVersionNumber,
                                })}
                              >
                                {t('instance.updateBadge', {
                                  version: update.latestVersionNumber,
                                })}
                              </span>
                            ) : null}
                          </div>
                          <span className={styles.contentMeta}>
                            {t(CONTENT_TYPE_KEYS[entry.type])} · {entry.provider}
                          </span>
                        </div>
                      </div>
                      <GlassButton
                        variant="ghost"
                        size="sm"
                        icon={<Trash2 size={13} aria-hidden />}
                        disabled={removeContent.isPending || isGameRunning}
                        onClick={() => handleRemoveContentEntry(entry.provider, entry.projectId)}
                        aria-label={t('instance.contentRemove')}
                        title={t('instance.contentRemove')}
                      >
                        {t('instance.contentRemove')}
                      </GlassButton>
                    </li>
                  )
                })}
              </ul>
            )}
          </GlassCard>
        ) : null}

        {/* ===================== TAB 7: SETTINGS ===================== */}
        {activeTab === 'settings' ? (
          <GlassCard tone="strong" className={styles.card}>
            <div className={styles.field}>
              <label htmlFor="inst-edit-name" className={styles.fieldLabel}>
                {t('instance.name')}
              </label>
              <input
                id="inst-edit-name"
                type="text"
                className={styles.filterInput}
                style={{ paddingLeft: '10px', width: '100%', maxWidth: '400px' }}
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                placeholder={t('instance.namePlaceholder')}
              />
            </div>

            <div className={styles.field}>
              <label htmlFor="inst-edit-version" className={styles.fieldLabel}>
                {t('instance.version')}
              </label>
              <div style={{ maxWidth: '400px' }}>
                <GlassSelect
                  id="inst-edit-version"
                  value={editVersion}
                  onValueChange={setEditVersion}
                  options={versionOptions}
                  placeholder={t('instance.versionUnavailable')}
                  searchable
                  testId="instance-edit-version-select"
                />
              </div>
            </div>

            <div className={styles.field}>
              <label htmlFor="inst-edit-loader" className={styles.fieldLabel}>
                {t('instance.loader')}
              </label>
              <div style={{ maxWidth: '400px' }}>
                <GlassSelect
                  id="inst-edit-loader"
                  value={editLoaderType}
                  onValueChange={(val) => {
                    setEditLoaderType(val as LoaderType)
                    setEditLoaderVersion(null)
                  }}
                  options={loaderOptions}
                  testId="instance-edit-loader-select"
                />
              </div>
            </div>

            <div style={{ display: 'flex', gap: '10px', marginTop: '12px' }}>
              <GlassButton
                variant="primary"
                onClick={handleSaveSettings}
                disabled={updateInstance.isPending}
                data-testid="instance-save-settings-btn"
              >
                {updateInstance.isPending ? t('common.loading') : t('common.save')}
              </GlassButton>
            </div>
          </GlassCard>
        ) : null}
      </div>

      <GlassDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={t('instances.deleteTitle')}
        description={t('instances.deleteBody', { name: instance.name })}
        closeLabel={t('common.close')}
        size="sm"
        footer={
          <>
            <GlassButton variant="ghost" onClick={() => setDeleteOpen(false)}>
              {t('common.cancel')}
            </GlassButton>
            <GlassButton
              variant="danger"
              disabled={removeInstance.isPending}
              onClick={handleDelete}
              data-testid="instance-delete-confirm"
            >
              {t('instances.delete')}
            </GlassButton>
          </>
        }
      >
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={deleteGameData}
            disabled={removeInstance.isPending}
            onChange={(e) => setDeleteGameData(e.target.checked)}
          />
          <span>{t('instances.deleteGameData')}</span>
        </label>
        <p className={styles.rowHint}>{t('instances.deleteGameDataHint')}</p>
        {instance.id === 'default' ? (
          <p className={styles.rowHint}>{t('instances.deleteDefaultNote')}</p>
        ) : null}
      </GlassDialog>

      {/* Dialog Tambah / Edit Server */}
      <GlassDialog
        open={serverDialogOpen}
        onOpenChange={setServerDialogOpen}
        title={
          serverDialogMode === 'add'
            ? t('instance.addServer')
            : t('instance.editServer')
        }
        closeLabel={t('common.close')}
      >
        <form onSubmit={handleSaveServer} className={styles.serverForm}>
          <div className={styles.formGroup}>
            <label className={styles.formLabel}>{t('instance.serverName')}</label>
            <input
              type="text"
              className={styles.formInput}
              placeholder={t('instance.serverNamePlaceholder')}
              value={serverNameInput}
              onChange={(e) => setServerNameInput(e.target.value)}
              required
              autoFocus
            />
          </div>
          <div className={styles.formGroup}>
            <label className={styles.formLabel}>{t('instance.serverIp')}</label>
            <input
              type="text"
              className={styles.formInput}
              placeholder={t('instance.serverIpPlaceholder')}
              value={serverIpInput}
              onChange={(e) => setServerIpInput(e.target.value)}
              required
            />
          </div>
          <div className={styles.formGroup}>
            <label className={styles.formLabel}>{t('instance.serverResourcePack')}</label>
            <GlassSelect
              value={serverResourcePackInput}
              onValueChange={(v) => setServerResourcePackInput(v as ServerResourcePackPolicy)}
              options={resourcePackOptions}
            />
          </div>
          <label className={styles.serverCheckboxRow}>
            <input
              type="checkbox"
              checked={serverHideFromHomeInput}
              onChange={(e) => setServerHideFromHomeInput(e.target.checked)}
            />
            <span>{t('instance.serverHideFromHome')}</span>
          </label>
          <div className={styles.serverDialogActions}>
            <button
              type="button"
              className={styles.serverCancelModalBtn}
              onClick={() => setServerDialogOpen(false)}
            >
              <X size={15} aria-hidden />
              <span>{t('common.cancel')}</span>
            </button>
            <button
              type="submit"
              className={styles.serverSaveModalBtn}
              disabled={
                !serverNameInput.trim() ||
                !serverIpInput.trim() ||
                addServer.isPending ||
                editServer.isPending
              }
            >
              <Save size={15} aria-hidden />
              <span>
                {serverDialogMode === 'add'
                  ? t('instance.addServer')
                  : t('instance.serverSaveChanges')}
              </span>
            </button>
          </div>
        </form>
      </GlassDialog>

      {/* Dialog Konfirmasi Hapus Server */}
      <GlassDialog
        open={serverDeleteTarget !== null}
        onOpenChange={(open) => !open && setServerDeleteTarget(null)}
        title={t('instance.deleteServer')}
        closeLabel={t('common.close')}
      >
        <p className={styles.deleteConfirmText}>
          {t('instance.deleteServerConfirm', {
            name: serverDeleteTarget?.name ?? '',
          })}
        </p>
        <div className={styles.serverDialogActions}>
          <GlassButton
            type="button"
            variant="ghost"
            onClick={() => setServerDeleteTarget(null)}
          >
            {t('common.cancel')}
          </GlassButton>
          <GlassButton
            type="button"
            variant="danger"
            disabled={removeServer.isPending}
            onClick={() => void handleConfirmDeleteServer()}
          >
            {t('instance.deleteServer')}
          </GlassButton>
        </div>
      </GlassDialog>
    </div>
  )
}
