import type { JvmPreset } from './instance'

export type ThemeMode = 'system' | 'dark' | 'light'

export type Language = 'id' | 'en'

export interface ThemeSettings {
  mode: ThemeMode
  reduceTransparency: boolean
  reduceMotion: boolean
}

export interface MemorySettings {
  defaultMaxMb: number
  defaultMinMb: number
}

export interface JavaSettings {
  preferManaged: boolean
  customPath: string | null
  jvmPreset: JvmPreset
  customJvmArgs: string
}

export interface DownloadSettings {
  parallelism: number
}

export interface DiscordSettings {
  enabled: boolean
  showIdle: boolean
  showPlaying: boolean
  customClientId: string | null
}

export interface Settings {
  schemaVersion: 1
  theme: ThemeSettings
  memory: MemorySettings
  java: JavaSettings
  downloads: DownloadSettings
  discord: DiscordSettings
  language: Language
  activeAccountId: string | null
}

export type SettingsPatch = {
  theme?: ThemeSettingsPatch | undefined
  memory?: MemorySettingsPatch | undefined
  java?: JavaSettingsPatch | undefined
  downloads?: DownloadSettingsPatch | undefined
  discord?: DiscordSettingsPatch | undefined
  language?: Language | undefined
  activeAccountId?: string | null | undefined
}

export interface ThemeSettingsPatch {
  mode?: ThemeMode | undefined
  reduceTransparency?: boolean | undefined
  reduceMotion?: boolean | undefined
}

export interface MemorySettingsPatch {
  defaultMaxMb?: number | undefined
  defaultMinMb?: number | undefined
}

export interface JavaSettingsPatch {
  preferManaged?: boolean | undefined
  customPath?: string | null | undefined
  jvmPreset?: JvmPreset | undefined
  customJvmArgs?: string | undefined
}

export interface DownloadSettingsPatch {
  parallelism?: number | undefined
}

export interface DiscordSettingsPatch {
  enabled?: boolean | undefined
  showIdle?: boolean | undefined
  showPlaying?: boolean | undefined
  customClientId?: string | null | undefined
}

export const DEFAULT_THEME: ThemeSettings = {
  mode: 'dark',
  reduceTransparency: false,
  reduceMotion: false,
}

export const DEFAULT_MEMORY: MemorySettings = {
  defaultMaxMb: 4096,
  defaultMinMb: 1024,
}

export const DEFAULT_JAVA: JavaSettings = {
  preferManaged: true,
  customPath: null,
  jvmPreset: 'default',
  customJvmArgs: '',
}

export const DEFAULT_DOWNLOADS: DownloadSettings = {
  parallelism: 8,
}

export const DEFAULT_DISCORD: DiscordSettings = {
  enabled: true,
  showIdle: true,
  showPlaying: true,
  customClientId: null,
}

export const DEFAULT_SETTINGS: Settings = {
  schemaVersion: 1,
  theme: DEFAULT_THEME,
  memory: DEFAULT_MEMORY,
  java: DEFAULT_JAVA,
  downloads: DEFAULT_DOWNLOADS,
  discord: DEFAULT_DISCORD,
  language: 'id',
  activeAccountId: null,
}

export function defaultSettings(): Settings {
  return {
    schemaVersion: 1,
    theme: { ...DEFAULT_THEME },
    memory: { ...DEFAULT_MEMORY },
    java: { ...DEFAULT_JAVA },
    downloads: { ...DEFAULT_DOWNLOADS },
    discord: { ...DEFAULT_DISCORD },
    language: DEFAULT_SETTINGS.language,
    activeAccountId: null,
  }
}
