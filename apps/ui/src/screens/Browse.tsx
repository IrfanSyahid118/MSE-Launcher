import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import {
  ArrowLeft,
  Check,
  ChevronLeft,
  ChevronRight,
  Download,
  ExternalLink,
  Search,
  Plus,
} from 'lucide-react'
import type {
  ContentProjectSummary,
  ContentProviderId,
  ContentSort,
  ContentType,
  ContentVersion,
} from '@mc/shared'
import { compareMinecraftVersions } from '@mc/shared'
import { useI18n } from '../i18n'
import type { MessageKey } from '../i18n'
import { errorMessage, invoke } from '../api/launcher'
import {
  useContentProject,
  useContentSearch,
  useContentVersions,
  useInstances,
  useIntegrations,
} from '../api/queries'
import { useAppStore } from '../stores/app-store'
import type { BreadcrumbSegment } from '../stores/app-store'
import { ContentInstallDialog } from '../components/ContentInstallDialog/ContentInstallDialog'
import { RightSidebar } from '../components/RightSidebar/RightSidebar'
import styles from './Browse.module.css'

const SEARCH_DEBOUNCE_MS = 300
const PROJECT_VERSIONS_PER_PAGE = 10

const CATEGORY_TABS: { id: ContentType; labelKey: MessageKey }[] = [
  { id: 'modpack', labelKey: 'browse.tab.modpack' },
  { id: 'mod', labelKey: 'browse.tab.mod' },
  { id: 'resourcepack', labelKey: 'browse.tab.resourcepack' },
  { id: 'datapack', labelKey: 'browse.tab.datapack' },
  { id: 'shader', labelKey: 'browse.tab.shader' },
  { id: 'world', labelKey: 'browse.tab.world' },
]

const SORT_OPTIONS: { id: ContentSort; labelKey: MessageKey }[] = [
  { id: 'relevance', labelKey: 'browse.sort.relevance' },
  { id: 'downloads', labelKey: 'browse.sort.downloads' },
  { id: 'follows', labelKey: 'browse.sort.follows' },
  { id: 'updated', labelKey: 'browse.sort.updated' },
  { id: 'newest', labelKey: 'browse.sort.newest' },
]

const PAGE_SIZES = [20, 50, 100]

const PROVIDER_LABEL: Record<ContentProviderId, string> = {
  modrinth: 'Modrinth',
  curseforge: 'CurseForge',
}

const CURSEFORGE_SECTION: Record<ContentType, string> = {
  mod: 'mc-mods',
  modpack: 'modpacks',
  resourcepack: 'texture-packs',
  shader: 'shaders',
  datapack: 'data-packs',
  world: 'worlds',
}

function projectExternalUrl(provider: ContentProviderId, type: ContentType, slug: string): string {
  const safeSlug = encodeURIComponent(slug)
  if (provider === 'modrinth') {
    return `https://modrinth.com/project/${safeSlug}`
  }
  return `https://www.curseforge.com/minecraft/${CURSEFORGE_SECTION[type]}/${safeSlug}`
}

const TYPE_LABEL_KEY: Record<ContentType, MessageKey> = {
  modpack: 'browse.modpackBadge',
  mod: 'browse.tab.mod',
  resourcepack: 'browse.tab.resourcepack',
  datapack: 'browse.tab.datapack',
  shader: 'browse.tab.shader',
  world: 'browse.tab.world',
}

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs)
    return () => window.clearTimeout(timer)
  }, [delayMs, value])
  return debounced
}

function formatCount(value: number): string {
  if (!value) return '0'
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

function relativeTime(iso: string | null | undefined): string | null {
  if (!iso) return null
  const time = Date.parse(iso)
  if (Number.isNaN(time)) return null
  const days = Math.floor((Date.now() - time) / 86_400_000)
  if (days < 1) return 'today'
  if (days === 1) return 'yesterday'
  if (days < 30) return `${days} days ago`
  const months = Math.round(days / 30)
  if (months < 12) return months === 1 ? 'last month' : `${months} months ago`
  const years = Math.round(days / 365)
  return years <= 1 ? 'last year' : `${years} years ago`
}

type BodyBlock =
  | { kind: 'heading'; text: string }
  | { kind: 'paragraph'; text: string }
  | { kind: 'bullet'; text: string }

/**
 * Markdown proyek dari provider dirender sebagai blok teks sederhana.
 * Gambar, HTML, dan tautan mentah dibuang supaya tidak ada konten asing yang
 * ikut tampil di dalam aplikasi.
 */
function parseProjectBody(body: string): BodyBlock[] {
  const blocks: BodyBlock[] = []
  const paragraph: string[] = []

  const clean = (value: string): string =>
    value
      .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
      .replace(/\[([^\]^]+)\]\[[^\]]*\]/g, '$1')
      .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/\[[^\]]{0,3}\]/g, '')
      .replace(/\[\^[^\]]*\]/g, '')
      .replace(/<[^>]+>/g, '')
      .replace(/[*_`>]/g, '')
      .replace(/\r/g, '')
      .replace(/\s{2,}/g, ' ')
      .trim()

  const flush = (): void => {
    if (paragraph.length === 0) return
    const text = clean(paragraph.join(' '))
    paragraph.length = 0
    if (text) blocks.push({ kind: 'paragraph', text })
  }

  for (const rawLine of body.split('\n')) {
    const line = rawLine.trim()
    if (!line) {
      flush()
      continue
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(line)
    if (heading) {
      flush()
      const text = clean(heading[1] ?? '')
      if (text) blocks.push({ kind: 'heading', text })
      continue
    }
    if (/^[-*+]\s+/.test(line)) {
      flush()
      const text = clean(line.replace(/^[-*+]\s+/, ''))
      if (text) blocks.push({ kind: 'bullet', text })
      continue
    }
    if (/^([-*_]\s*){3,}$/.test(line)) {
      flush()
      continue
    }
    paragraph.push(line)
  }
  flush()
  return blocks.slice(0, 120)
}

function ProjectBody({ body }: { body: string }): ReactNode {
  const { t } = useI18n()
  const blocks = useMemo(() => parseProjectBody(body), [body])

  if (blocks.length === 0) {
    return <p className={styles.bodyText}>{t('browse.noDescriptionYet')}</p>
  }

  return (
    <div className={styles.markdownBody}>
      {blocks.map((block, index) => {
        if (block.kind === 'heading') {
          return (
            <h2 key={index} className={styles.bodyHeading}>
              {block.text}
            </h2>
          )
        }
        if (block.kind === 'bullet') {
          return (
            <p key={index} className={styles.bodyBullet}>
              <span aria-hidden>•</span>
              <span>{block.text}</span>
            </p>
          )
        }
        return (
          <p key={index} className={styles.bodyText}>
            {block.text}
          </p>
        )
      })}
    </div>
  )
}

/** Deret nomor halaman dengan elipsis di tengah. */
function pageNumbers(page: number, totalPages: number): (number | 'gap')[] {  if (totalPages <= 7) {
    return Array.from({ length: totalPages }, (_, index) => index + 1)
  }
  const pages = new Set<number>([1, totalPages, page, page - 1, page + 1])
  const sorted = [...pages].filter((value) => value >= 1 && value <= totalPages).sort((a, b) => a - b)
  const result: (number | 'gap')[] = []
  sorted.forEach((value, index) => {
    if (index > 0 && value - (sorted[index - 1] as number) > 1) {
      result.push('gap')
    }
    result.push(value)
  })
  return result
}

export function Browse(): ReactNode {
  const { t } = useI18n()
  const pushToast = useAppStore((state) => state.pushToast)
  const instancesQuery = useInstances()
  const instances = useMemo(() => instancesQuery.data ?? [], [instancesQuery.data])
  const integrationsQuery = useIntegrations()
  const curseForgeReady = integrationsQuery.data?.curseforge.configured ?? false
  const setBreadcrumb = useAppStore((state) => state.setBreadcrumb)

  const [provider, setProvider] = useState<ContentProviderId>('modrinth')
  const [activeTab, setActiveTab] = useState<ContentType>('modpack')
  const [text, setText] = useState('')
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [sort, setSort] = useState<ContentSort>('relevance')
  const [categories, setCategories] = useState<string[]>([])
  const [hideInstalled, setHideInstalled] = useState(false)

  const [activeProject, setActiveProject] = useState<ContentProjectSummary | null>(null)
  const [projectTab, setProjectTab] = useState<'description' | 'versions'>('description')
  const [activeVersion, setActiveVersion] = useState<ContentVersion | null>(null)
  const [channel, setChannel] = useState<'all' | 'release' | 'beta' | 'alpha'>('all')
  const [gameVersion, setGameVersion] = useState('all')
  const [versionPage, setVersionPage] = useState(1)

  const [installDialogItem, setInstallDialogItem] = useState<ContentProjectSummary | null>(null)
  const [installDialogVersionId, setInstallDialogVersionId] = useState<string | null>(null)

  const openInstallDialog = (item: ContentProjectSummary, versionId: string | null = null): void => {
    setInstallDialogVersionId(versionId)
    setInstallDialogItem(item)
  }

  const debouncedText = useDebouncedValue(text.trim(), SEARCH_DEBOUNCE_MS)
  const selectedInstanceId = useAppStore((state) => state.selectedInstanceId)
  const setSelectedInstance = useAppStore((state) => state.setSelectedInstance)
  const instance = useMemo(
    () => instances.find((i) => i.id === selectedInstanceId) ?? instances[0] ?? null,
    [instances, selectedInstanceId],
  )

  const isModpack = activeTab === 'modpack'
  const isBlocked =
    (activeTab === 'world' && provider === 'modrinth') ||
    (provider === 'curseforge' && !curseForgeReady) ||
    (!isModpack && !instance)

  const searchQuery = useMemo(
    () => ({
      provider,
      type: activeTab,
      text: debouncedText || '',
      page,
      pageSize,
      sort,
      minecraftVersion: !isModpack && instance ? instance.minecraftVersion : undefined,
      loader:
        activeTab === 'mod' && instance && instance.loader.type !== 'vanilla'
          ? instance.loader.type
          : undefined,
      categories: provider === 'modrinth' && categories.length > 0 ? categories : undefined,
    }),
    [activeTab, categories, debouncedText, instance, isModpack, page, pageSize, provider, sort],
  )

  const searchResults = useContentSearch(isBlocked ? null : searchQuery)
  const allItems = useMemo(() => searchResults.data?.items ?? [], [searchResults.data])
  const total = searchResults.data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / pageSize))

  const items = useMemo(() => {
    if (!hideInstalled || !instance) return allItems
    return allItems.filter(
      (item) => !instance.content.some((entry) => entry.projectId === item.id),
    )
  }, [allItems, hideInstalled, instance])

  const versionsQuery = useContentVersions(
    activeProject?.provider ?? provider,
    activeProject?.id ?? '',
    {},
    Boolean(activeProject),
  )
  const projectQuery = useContentProject(
    activeProject?.provider ?? provider,
    activeProject?.id ?? null,
  )
  const projectDetail = projectQuery.data ?? null
  const allVersions = useMemo(() => versionsQuery.data ?? [], [versionsQuery.data])

  const gameVersionOptions = useMemo(() => {
    const values = new Set<string>()
    for (const version of allVersions) {
      for (const value of version.gameVersions) values.add(value)
    }
    return [...values]
  }, [allVersions])

  const versions = useMemo(() => {
    return allVersions.filter((version) => {
      if (channel !== 'all' && version.versionType !== channel) return false
      if (gameVersion !== 'all' && !version.gameVersions.includes(gameVersion)) return false
      return true
    })
  }, [allVersions, channel, gameVersion])

  const versionTotalPages = Math.max(1, Math.ceil(versions.length / PROJECT_VERSIONS_PER_PAGE))
  const visibleVersions = versions.slice(
    (versionPage - 1) * PROJECT_VERSIONS_PER_PAGE,
    versionPage * PROJECT_VERSIONS_PER_PAGE,
  )

  const compatibility = useMemo(() => {
    const sortVersions = (values: string[]): string[] =>
      [...values].sort((a, b) => compareMinecraftVersions(b, a))
    if (projectDetail) {
      return {
        gameVersions: sortVersions(projectDetail.gameVersions),
        platforms: projectDetail.loaders.map(
          (value) => value.charAt(0).toUpperCase() + value.slice(1),
        ),
      }
    }
    const gameVersions = new Set<string>()
    const platforms = new Set<string>()
    for (const version of allVersions) {
      for (const value of version.gameVersions) gameVersions.add(value)
      for (const value of version.loaders) platforms.add(value)
    }
    return {
      gameVersions: sortVersions([...gameVersions]),
      platforms: [...platforms].map((value) => value.charAt(0).toUpperCase() + value.slice(1)),
    }
  }, [allVersions, projectDetail])

  const latestPublished = allVersions[0]?.publishedAt ?? null

  useEffect(() => {
    setPage(1)
  }, [provider, activeTab, sort, debouncedText, pageSize, categories])

  useEffect(() => {
    setVersionPage(1)
  }, [channel, gameVersion, activeProject?.id])

  useEffect(() => {
    if (!activeProject) {
      setBreadcrumb(null)
      return
    }
    const trail: BreadcrumbSegment[] = [
      {
        label: t('nav.browse'),
        onClick: () => {
          setActiveProject(null)
          setActiveVersion(null)
        },
      },
      activeVersion
        ? {
            label: activeProject.title,
            onClick: () => setActiveVersion(null),
          }
        : { label: activeProject.title },
    ]
    if (activeVersion) {
      trail.push({ label: `Version ${activeVersion.name}` })
    }
    setBreadcrumb(trail)
    return () => setBreadcrumb(null)
  }, [activeProject, activeVersion, setBreadcrumb])

  const toggleCategory = (category: string): void => {
    setCategories((prev) =>
      prev.includes(category) ? prev.filter((entry) => entry !== category) : [...prev, category],
    )
  }

  const openProject = (project: ContentProjectSummary): void => {
    setActiveProject(project)
    setActiveVersion(null)
    setProjectTab('description')
  }

  const isInstalled = (project: ContentProjectSummary): boolean =>
    Boolean(
      instance?.content.some(
        (entry) => entry.projectId === project.id && entry.provider === project.provider,
      ),
    )

  const needsInstance = (item: ContentProjectSummary): boolean =>
    item.type !== 'modpack' && !instance

  const renderNotice = (): ReactNode => {
    if (activeTab === 'world' && provider === 'modrinth') {
      return (
        <div className={styles.notice} data-testid="browse-world-needs-curseforge">
          <h2>{t('browse.worldNeedsCurseforge')}</h2>
          <p>{t('browse.worldNeedsCurseforgeHint')}</p>
        </div>
      )
    }
    if (provider === 'curseforge' && !curseForgeReady) {
      return (
        <div className={styles.notice} data-testid="browse-curseforge-missing">
          <h2>{t('browse.curseforgeMissing')}</h2>
          <p>{t('browse.curseforgeMissingHint')}</p>
        </div>
      )
    }
    if (!isModpack && !instance) {
      return (
        <div className={styles.notice} data-testid="browse-no-instance">
          <h2>{t('browse.noInstances')}</h2>
          <p>{t('browse.noInstancesHint')}</p>
        </div>
      )
    }
    return null
  }

  const pager = (current: number, totalCount: number, onChange: (next: number) => void): ReactNode => {
    if (totalCount <= 1) return null
    return (
      <nav className={styles.pager} aria-label={t('browse.paginationAria')}>
        <button
          type="button"
          className={styles.pagerArrow}
          onClick={() => onChange(Math.max(1, current - 1))}
          disabled={current === 1}
          aria-label={t('browse.prevPage')}
        >
          <ChevronLeft size={15} aria-hidden />
        </button>
        {pageNumbers(current, totalCount).map((entry, index) =>
          entry === 'gap' ? (
            <span key={`gap-${index}`} className={styles.pagerGap} aria-hidden>
              …
            </span>
          ) : (
            <button
              key={entry}
              type="button"
              className={`${styles.pagerButton} ${entry === current ? styles.pagerButtonActive : ''}`}
              onClick={() => onChange(entry)}
              aria-current={entry === current ? 'page' : undefined}
            >
              {entry}
            </button>
          ),
        )}
        <button
          type="button"
          className={styles.pagerArrow}
          onClick={() => onChange(Math.min(totalCount, current + 1))}
          disabled={current === totalCount}
          aria-label={t('browse.nextPage')}
        >
          <ChevronRight size={15} aria-hidden />
        </button>
      </nav>
    )
  }

  const activeTabLabel = t(CATEGORY_TABS.find((tab) => tab.id === activeTab)?.labelKey ?? 'browse.tab.mod')

  return (
    <div className={styles.page} data-testid="browse-screen">
      <div className={styles.container}>
        <div className={styles.mainContent}>
          {!activeProject ? (
            <>
              <div className={styles.topRow}>
                <div className={styles.tabs} role="tablist" aria-label={t('browse.contentTypeAria')}>
                  {CATEGORY_TABS.map((tab) => {
                    const isActive = activeTab === tab.id
                    return (
                      <button
                        key={tab.id}
                        type="button"
                        role="tab"
                        aria-selected={isActive}
                        data-testid={`browse-tab-${tab.id}`}
                        className={`${styles.tab} ${isActive ? styles.tabActive : ''}`}
                        onClick={() => {
                          setActiveTab(tab.id)
                          setCategories([])
                        }}
                      >
                        {t(tab.labelKey)}
                      </button>
                    )
                  })}
                </div>

                <div className={styles.providerPills} role="group" aria-label={t('browse.contentProviderAria')}>
                  {(['modrinth', 'curseforge'] as ContentProviderId[]).map((id) => (
                    <button
                      key={id}
                      type="button"
                      data-testid={`browse-provider-${id}`}
                      aria-pressed={provider === id}
                      className={`${styles.providerBtn} ${provider === id ? styles.providerBtnActive : ''}`}
                      onClick={() => setProvider(id)}
                    >
                      {PROVIDER_LABEL[id]}
                    </button>
                  ))}
                </div>
              </div>

              <div className={styles.searchBarWrapper}>
                <Search size={15} className={styles.searchIcon} aria-hidden />
                <input
                  type="text"
                  className={styles.searchBarInput}
                  placeholder={t('browse.searchTypePlaceholder', { noun: activeTabLabel.toLowerCase() })}
                  value={text}
                  disabled={Boolean(isBlocked)}
                  onChange={(event) => setText(event.target.value)}
                  data-testid="browse-search-input"
                  aria-label={t('browse.searchContentAria')}
                />
              </div>

              <div className={styles.controlBar}>
                <div className={styles.controlBarLeft}>
                  {!isModpack && instances.length > 0 ? (
                    <label className={styles.controlLabel}>
                      <span>{t('browse.target')}</span>
                      <select
                        className={styles.select}
                        value={instance?.id ?? ''}
                        onChange={(event) => setSelectedInstance(event.target.value)}
                        data-testid="browse-target-instance"
                      >
                        {instances.map((inst) => (
                          <option key={inst.id} value={inst.id}>
                            {inst.name} ({inst.minecraftVersion} · {inst.loader.type})
                          </option>
                        ))}
                      </select>
                    </label>
                  ) : null}

                  <label className={styles.controlLabel}>
                    <span>{t('browse.sortLabel')}</span>
                    <select
                      className={styles.select}
                      value={sort}
                      onChange={(event) => setSort(event.target.value as ContentSort)}
                    >
                      {SORT_OPTIONS.map((option) => (
                        <option key={option.id} value={option.id}>
                          {t(option.labelKey)}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className={styles.controlLabel}>
                    <span>{t('browse.viewLabel')}</span>
                    <select
                      className={styles.select}
                      value={pageSize}
                      onChange={(event) => setPageSize(Number(event.target.value))}
                    >
                      {PAGE_SIZES.map((size) => (
                        <option key={size} value={size}>
                          {size}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                {pager(page, totalPages, setPage)}
              </div>

              {renderNotice() ?? (
                <>
                  {searchResults.isLoading || searchResults.isPlaceholderData ? (
                    <div className={styles.rows} aria-busy="true" aria-label={t('browse.loadingResults')}>
                      {Array.from({ length: 5 }, (_, index) => (
                        <div key={index} className={styles.skeletonRow} />
                      ))}
                    </div>
                  ) : searchResults.isError ? (
                    <div className={styles.notice}>
                      <h2>{t('browse.searchFailed')}</h2>
                      <p>{t('browse.searchFailedHint')}</p>
                      <button
                        type="button"
                        className={styles.primaryButton}
                        onClick={() => void searchResults.refetch()}
                      >
                        {t('common.retry')}
                      </button>
                    </div>
                  ) : items.length === 0 ? (
                    <div className={styles.notice}>
                      <h2>{t('browse.noResultsForTab', { noun: activeTabLabel.toLowerCase() })}</h2>
                      <p>
                        {categories.length > 0 || text.trim()
                          ? t('browse.tryAnotherKeyword')
                          : t('browse.nothingMatches')}
                      </p>
                      {categories.length > 0 ? (
                        <button
                          type="button"
                          className={styles.secondaryButton}
                          onClick={() => setCategories([])}
                        >
                          {t('browse.clearCategoryFilters')}
                        </button>
                      ) : null}
                    </div>
                  ) : (
                    <div className={styles.rows} data-testid="browse-results">
                      {items.map((item) => {
                        const installed = isInstalled(item)
                        const blocked = needsInstance(item)
                        return (
                          <article key={`${item.provider}-${item.id}`} className={styles.row}>
                            <button
                              type="button"
                              className={styles.rowSurface}
                              onClick={() => openProject(item)}
                              aria-label={`Open ${item.title}`}
                            />
                            <div className={styles.rowIcon}>
                              {item.iconUrl ? (
                                <img
                                  src={item.iconUrl}
                                  alt=""
                                  width={56}
                                  height={56}
                                  loading="lazy"
                                  decoding="async"
                                />
                              ) : (
                                <span aria-hidden>{item.title.slice(0, 2).toUpperCase()}</span>
                              )}
                            </div>

                            <div className={styles.rowBody}>
                              <div className={styles.rowTitleLine}>
                                <h3 className={styles.rowTitle}>{item.title}</h3>
                                <span className={styles.rowAuthor}>{t('browse.byAuthor', { author: item.slug })}</span>
                              </div>
                              <p className={styles.rowDescription}>{item.description}</p>
                              <div className={styles.rowTags}>
                                <span className={styles.tagMuted}>{t(TYPE_LABEL_KEY[item.type])}</span>
                                <span className={styles.tagMuted}>{PROVIDER_LABEL[item.provider]}</span>
                                {installed ? (
                                  <span className={styles.tagInstalled}>
                                    <Check size={11} aria-hidden /> {t('browse.installedBadge')}
                                  </span>
                                ) : null}
                              </div>
                            </div>

                            <div className={styles.rowActions}>
                              <button
                                type="button"
                                className={styles.installButton}
                                onClick={() => openInstallDialog(item)}
                                disabled={blocked}
                                title={blocked ? t('browse.createInstanceFirst') : undefined}
                                data-testid="content-install-button"
                              >
                                <Plus size={13} strokeWidth={2.6} aria-hidden />
                                <span>{installed ? t('browse.reinstall') : t('browse.install')}</span>
                              </button>
                              <span className={styles.rowStats}>
                                <Download size={12} aria-hidden />
                                {formatCount(item.downloads)}
                              </span>
                            </div>
                          </article>
                        )
                      })}
                    </div>
                  )}
                </>
              )}
            </>
          ) : activeVersion ? (
            /* Detail versi (gambar 10) */
            <div className={styles.versionDetail}>
              <button
                type="button"
                className={styles.backLink}
                onClick={() => setActiveVersion(null)}
              >
                <ArrowLeft size={14} aria-hidden />
                <span>{t('browse.allVersions')}</span>
              </button>

              <div className={styles.versionHeader}>
                <div className={styles.versionHeading}>
                  <h1 className={styles.versionTitle}>{activeVersion.name}</h1>
                  <span
                    className={`${styles.channelBadge} ${styles[`channel_${activeVersion.versionType}`]}`}
                  >
                    {activeVersion.versionType}
                  </span>
                </div>
                <div className={styles.versionMeta}>
                  <span>
                    {activeVersion.gameVersions[0] ?? 'Minecraft'} ·{' '}
                    {relativeTime(activeVersion.publishedAt) ?? t('common.unknown')}
                  </span>
                  <span className={styles.metaDot}>•</span>
                  <span>
                    <Download size={12} aria-hidden /> {formatCount(activeVersion.downloads)}
                  </span>
                </div>

                <button
                  type="button"
                  className={styles.primaryButton}
                  onClick={() => openInstallDialog(activeProject, activeVersion.id)}
                  disabled={activeVersion.downloadUrl === null}
                  title={
                    activeVersion.downloadUrl === null
                      ? t('browse.thirdPartyBlocked')
                      : undefined
                  }
                >
                  <Download size={14} aria-hidden />
                  <span>{t('browse.install')}</span>
                </button>
              </div>

              <div className={styles.infoGrid}>
                <section className={styles.infoCard}>
                  <h2 className={styles.infoTitle}>{t('browse.compatibility')}</h2>
                  <span className={styles.infoLabel}>Minecraft: Java Edition</span>
                  <div className={styles.chips}>
                    {activeVersion.gameVersions.map((value) => (
                      <span key={value} className={styles.chip}>
                        {value}
                      </span>
                    ))}
                  </div>
                  <span className={styles.infoLabel}>{t('browse.platforms')}</span>
                  <div className={styles.chips}>
                    {activeVersion.loaders.map((value) => (
                      <span key={value} className={styles.chip}>
                        {value}
                      </span>
                    ))}
                  </div>
                </section>

                <section className={styles.infoCard}>
                  <h2 className={styles.infoTitle}>{t('browse.fileSection')}</h2>
                  <span className={styles.infoLabel}>{t('browse.fileName')}</span>
                  <span className={styles.fileName}>{activeVersion.fileName}</span>
                  <span className={styles.infoLabel}>{t('browse.checksum')}</span>
                  <span className={styles.fileName}>
                    {activeVersion.sha1 ? `SHA-1 ${activeVersion.sha1.slice(0, 16)}…` : t('browse.notProvided')}
                  </span>
                </section>
              </div>
            </div>
          ) : (
            /* Detail proyek (gambar 3 & 4) */
            <div className={styles.projectView}>
              <header className={styles.projectHero}>
                <div className={styles.projectIcon}>
                  {activeProject.iconUrl ? (
                    <img
                      src={activeProject.iconUrl}
                      alt=""
                      width={64}
                      height={64}
                      loading="lazy"
                      decoding="async"
                    />
                  ) : (
                    <span aria-hidden>{activeProject.title.slice(0, 2).toUpperCase()}</span>
                  )}
                </div>

                <div className={styles.projectMeta}>
                  <div className={styles.projectTitleLine}>
                    <h1 className={styles.projectTitle}>{activeProject.title}</h1>
                    <span className={styles.rowAuthor}>{t('browse.byAuthor', { author: activeProject.slug })}</span>
                  </div>
                  <p className={styles.projectSummary}>{activeProject.description}</p>
                  <div className={styles.projectStats}>
                    <span>
                      <Download size={12} aria-hidden /> {formatCount(activeProject.downloads)}
                    </span>
                    {latestPublished ? (
                      <>
                        <span className={styles.metaDot}>•</span>
                        <span>{t('browse.updatedAt', { time: relativeTime(latestPublished) ?? '' })}</span>
                      </>
                    ) : null}
                    <span className={styles.tagMuted}>{t(TYPE_LABEL_KEY[activeProject.type])}</span>
                    <span className={styles.tagMuted}>
                      {PROVIDER_LABEL[activeProject.provider]}
                    </span>
                    {isInstalled(activeProject) ? (
                      <span className={styles.tagInstalled}>
                        <Check size={11} aria-hidden /> {t('browse.installedBadge')}
                      </span>
                    ) : null}
                  </div>
                </div>

                <button
                  type="button"
                  className={styles.primaryButton}
                  onClick={() => openInstallDialog(activeProject)}
                  disabled={needsInstance(activeProject)}
                  title={needsInstance(activeProject) ? t('browse.createInstanceFirst') : undefined}
                >
                  <Download size={14} aria-hidden />
                  <span>{isInstalled(activeProject) ? t('browse.reinstall') : t('browse.install')}</span>
                </button>
              </header>

              <div className={styles.projectTabs} role="tablist" aria-label={t('browse.projectSectionsAria')}>
                {(
                  [
                    { id: 'description', label: t('browse.tabDescription') },
                    { id: 'versions', label: t('browse.tabVersions') },
                  ] as const
                ).map((tab) => (
                  <button
                    key={tab.id}
                    type="button"
                    role="tab"
                    aria-selected={projectTab === tab.id}
                    className={`${styles.projectTab} ${projectTab === tab.id ? styles.projectTabActive : ''}`}
                    onClick={() => setProjectTab(tab.id)}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>

              {projectTab === 'description' ? (
                <div className={styles.descriptionPane}>
                  {projectQuery.isLoading ? (
                    <div className={styles.paragraphSkeleton} aria-busy="true" aria-label={t('browse.loadingDescription')} />
                  ) : projectQuery.isError || !projectDetail ? (
                    <>
                      <p className={styles.leadPara}>{activeProject.description}</p>
                      <p className={styles.bodyText}>
                        {t('browse.descriptionFailed')}
                      </p>
                    </>
                  ) : (
                    <ProjectBody body={projectDetail.body} />
                  )}

                  {projectDetail && projectDetail.categories.length > 0 ? (
                    <div className={styles.categoriesRow}>
                      {projectDetail.categories.map((category) => (
                        <span key={category} className={styles.tagMuted}>
                          {category}
                        </span>
                      ))}
                    </div>
                  ) : null}

                  <button
                    type="button"
                    className={styles.textLink}
                    onClick={() => {
                      const url = projectExternalUrl(
                        activeProject.provider,
                        activeProject.type,
                        activeProject.slug,
                      )
                      void invoke('shell:openExternal', { url }).catch((err: unknown) => {
                        pushToast(errorMessage(err, t('error.unknown')), 'error')
                      })
                    }}
                  >
                    <ExternalLink size={13} aria-hidden />
                    <span>Open on {PROVIDER_LABEL[activeProject.provider]}</span>
                  </button>
                </div>
              ) : projectTab === 'versions' ? (
                <div className={styles.versionsPane}>
                  <div className={styles.versionsToolbar}>
                    <div className={styles.versionsFilters}>
                      <label className={styles.controlLabel}>
                        <span>{t('browse.channel')}</span>
                        <select
                          className={styles.select}
                          value={channel}
                          onChange={(event) =>
                            setChannel(event.target.value as typeof channel)
                          }
                        >
                          <option value="all">{t('browse.channelAll')}</option>
                          <option value="release">{t('browse.channelRelease')}</option>
                          <option value="beta">{t('browse.channelBeta')}</option>
                          <option value="alpha">{t('browse.channelAlpha')}</option>
                        </select>
                      </label>

                      <label className={styles.controlLabel}>
                        <span>{t('browse.gameVersion')}</span>
                        <select
                          className={styles.select}
                          value={gameVersion}
                          onChange={(event) => setGameVersion(event.target.value)}
                        >
                          <option value="all">{t('browse.channelAll')}</option>
                          {gameVersionOptions.map((value) => (
                            <option key={value} value={value}>
                              {value}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    {pager(versionPage, versionTotalPages, setVersionPage)}
                  </div>

                  {versionsQuery.isLoading ? (
                    <div className={styles.rows} aria-busy="true" aria-label={t('browse.loadingVersions')}>
                      {Array.from({ length: 6 }, (_, index) => (
                        <div key={index} className={styles.skeletonRow} />
                      ))}
                    </div>
                  ) : versionsQuery.isError ? (
                    <div className={styles.notice}>
                      <h2>{t('browse.versionsFailed')}</h2>
                      <p>{t('browse.versionsFailedHint')}</p>
                      <button
                        type="button"
                        className={styles.primaryButton}
                        onClick={() => void versionsQuery.refetch()}
                      >
                        {t('common.retry')}
                      </button>
                    </div>
                  ) : versions.length === 0 ? (
                    <div className={styles.notice}>
                      <h2>{t('browse.noVersionsMatch')}</h2>
                      <p>{t('browse.pickAnotherFilter')}</p>
                    </div>
                  ) : (
                    <div className={styles.tableWrap}>
                      <table className={styles.table}>
                        <thead>
                          <tr>
                            <th scope="col">{t('browse.tableVersion')}</th>
                            <th scope="col">{t('browse.gameVersion')}</th>
                            <th scope="col">{t('browse.platforms')}</th>
                            <th scope="col">{t('browse.tablePublished')}</th>
                            <th scope="col">{t('browse.tableDownloads')}</th>
                            <th scope="col" className={styles.alignEnd}>
                              {t('browse.tableActions')}
                            </th>
                          </tr>
                        </thead>
                        <tbody>
                          {visibleVersions.map((version) => (
                            <tr key={version.id}>
                              <td>
                                <div className={styles.versionCell}>
                                  <span
                                    className={`${styles.channelDot} ${styles[`channel_${version.versionType}`]}`}
                                    aria-hidden
                                  />
                                  <button
                                    type="button"
                                    className={styles.versionLink}
                                    onClick={() => setActiveVersion(version)}
                                  >
                                    {version.name}
                                  </button>
                                </div>
                              </td>
                              <td>
                                <span className={styles.chip}>
                                  {version.gameVersions[0] ?? '-'}
                                </span>
                              </td>
                              <td>
                                <span className={styles.chip}>
                                  {version.loaders[0] ?? '-'}
                                </span>
                              </td>
                              <td className={styles.mutedCell}>
                                {relativeTime(version.publishedAt) ?? '-'}
                              </td>
                              <td className={styles.mutedCell}>{formatCount(version.downloads)}</td>
                              <td className={styles.alignEnd}>
                                <button
                                  type="button"
                                  className={styles.iconButton}
                                  onClick={() => openInstallDialog(activeProject, version.id)}
                                  aria-label={t('browse.installVersionAria', { name: version.name })}
                                  title={t('browse.installThisVersion')}
                                >
                                  <Download size={14} aria-hidden />
                                </button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              ) : null}
            </div>
          )}
        </div>

        <RightSidebar
          mode={activeProject ? 'project' : 'browse'}
          hideInstalled={hideInstalled}
          onToggleHideInstalled={setHideInstalled}
          selectedCategories={categories}
          onSelectCategory={toggleCategory}
          projectCompatibility={activeProject ? compatibility : undefined}
        />
      </div>

      {installDialogItem && (instance || installDialogItem.type === 'modpack') ? (
        <ContentInstallDialog
          open
          onOpenChange={(next) => {
            if (!next) {
              setInstallDialogItem(null)
              setInstallDialogVersionId(null)
            }
          }}
          project={installDialogItem}
          instance={instance}
          initialVersionId={installDialogVersionId}
        />
      ) : null}
    </div>
  )
}
