export type ContentType = 'mod' | 'modpack' | 'shader' | 'resourcepack' | 'datapack' | 'world'

export type ContentProviderId = 'modrinth' | 'curseforge'

/** Urutan hasil pencarian yang didukung kedua provider. */
export type ContentSort = 'relevance' | 'downloads' | 'follows' | 'updated' | 'newest'

export interface InstanceContentEntry {
  provider: ContentProviderId
  projectId: string
  versionId: string
  type: ContentType
  fileName: string
  sha1: string
  /**
   * Folder hasil ekstrak untuk konten `world` di `minecraft/saves`. World tidak
   * bisa dihapus dari nama arsipnya karena nama folder ditentukan isi zip.
   */
  worldFolder?: string | undefined
  /** Status nonaktif (file di-rename menjadi .disabled di disk). */
  disabled?: boolean | undefined
}

/** Jenis relasi dependensi yang sudah dinormalkan dari kedua provider. */
export type ContentDependencyKind =
  | 'required'
  | 'optional'
  | 'embedded'
  | 'incompatible'
  | 'tool'

export interface ContentDependency {
  projectId: string | null
  versionId: string | null
  kind: ContentDependencyKind
}

export interface ContentProjectSummary {
  provider: ContentProviderId
  id: string
  slug: string
  title: string
  description: string
  type: ContentType
  downloads: number
  iconUrl: string | null
}

export interface ContentProject extends ContentProjectSummary {
  body: string
  updatedAt: string
  categories: string[]
  loaders: string[]
  gameVersions: string[]
}

export interface ContentVersion {
  provider: ContentProviderId
  id: string
  projectId: string
  name: string
  versionNumber: string
  versionType: 'release' | 'beta' | 'alpha'
  gameVersions: string[]
  loaders: string[]
  downloads: number
  publishedAt: string
  fileName: string
  /** null jika provider melarang distribusi pihak ketiga. */
  downloadUrl: string | null
  sha1: string | null
}

export interface ContentFilter {
  minecraftVersion?: string | undefined
  loader?: string | undefined
  featured?: boolean | undefined
}

export interface SearchQuery {
  provider: ContentProviderId
  type: ContentType
  text: string
  page: number
  pageSize: number
  minecraftVersion?: string | undefined
  loader?: string | undefined
  /** Urutan hasil; provider menerjemahkannya ke indeks miliknya. */
  sort?: ContentSort | undefined
  /** Filter kategori (mis. Optimization, Adventure). */
  categories?: string[] | undefined
}

export interface SearchResult {
  items: ContentProjectSummary[]
  total: number
  page: number
  pageSize: number
}

export interface ResolvedDownload {
  url: string
  fileName: string
  sha1: string | null
  size: number | null
}

/**
 * Satu berkas dalam rencana pemasangan. Tipe dibaca dari proyeknya, bukan
 * diwarisi dari sasaran: shader Iris bergantung pada mod Sodium, sehingga
 * kedua berkas berakhir di folder berbeda.
 */
export interface ContentPlanEntry {
  version: ContentVersion
  type: ContentType
  /**
   * `false` bila provider tidak menyediakan URL unduhan (CurseForge melarang
   * distribusi pihak ketiga). UI menawarkan "Buka di CurseForge", bukan unduhan.
   */
  downloadable: boolean
}

/** Rencana pemasangan: sasaran yang dipilih ditambah dependensi wajibnya. */
export interface ContentInstallPlan {
  target: ContentPlanEntry
  dependencies: ContentPlanEntry[]
  /** Contoh: dependensi `incompatible` yang perlu diketahui pengguna. */
  warnings: string[]
}

/** Folder world di `minecraft/saves` yang sudah punya `level.dat`. */
export interface SavedWorld {
  folder: string
  name: string
}

/** Pembaruan konten yang tersedia untuk satu entri instance. */
export interface ContentUpdateInfo {
  provider: ContentProviderId
  projectId: string
  currentVersionId: string
  latestVersionId: string
  latestVersionNumber: string
  fileName: string
}

/** Permintaan pemasangan modpack menjadi instance baru. */
export interface InstallModpackRequest {
  provider: ContentProviderId
  versionId: string
  instanceName?: string | undefined
}


